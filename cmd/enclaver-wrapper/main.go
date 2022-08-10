package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/nitrocli"
	"github.com/go-edgebit/enclaver/policy"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"github.com/urfave/cli/v2"
	"io"
	"math"
	"math/rand"
	"os"
	"time"

	"go.uber.org/zap"
)

const (
	enclaveImagePath      = "/enclave/application.eif"
	egressProxyListenPort = 3128
)

func main() {
	app := &cli.App{
		Name:   "enclaver-wrapper",
		Usage:  "Start an enclaver application and proxy its traffic",
		Action: run,
		Flags: []cli.Flag{
			&cli.BoolFlag{
				Name:  "debug",
				Usage: "run enclave in debug mode and attach to its console",
			},
		},
	}

	err := app.Run(os.Args)
	if err != nil {
		fmt.Println("error: " + err.Error())
	}
}

func run(cliContext *cli.Context) error {
	ctx := context.Background()
	debugMode := cliContext.Bool("debug")

	policy, err := policy.LoadPolicy(policy.WrapperPolicyLocation)
	if err != nil {
		return err
	}

	parsedPolicy := policy.Parsed()

	logger, err := zap.NewProduction()
	if err != nil {
		return err
	}

	cid := uint32(rand.Int63n(math.MaxUint32-4) + 4)

	// TODO: load all ports from the app manifest
	pf := proxy.MakeParentForwarder(logger, "0.0.0.0", cid)

	for _, port := range parsedPolicy.Network.ListenPorts {
		err = pf.ForwardPort(context.Background(), uint32(port), uint32(port))
		if err != nil {
			return err
		}
	}

	listener, err := vsock.Listen(egressProxyListenPort)
	if err != nil {
		return err
	}

	httpProxy := proxy.MakeHTTPProxy(logger)

	go httpProxy.Serve(listener)

	cli := &nitrocli.NitroCLI{}
	enclaveOpts := nitrocli.RunEnclaveOptions{
		CPUCount: parsedPolicy.Resources.CPUs,
		Memory:   parsedPolicy.Resources.Mem,
		EIFPath:  enclaveImagePath,
		CID:      cid,
	}

	if debugMode {
		out, err := cli.RunEnclaveDebugConsole(ctx, enclaveOpts)
		if err != nil {
			logger.Error("error starting enclave in debug mode",
				zap.Error(err),
				zap.ByteString("output", out.Bytes()))
			return err
		}

		println("DEBUG OUTPUT")
		io.Copy(os.Stdout, out)

		return nil
	}

	enclaveInfo, err := cli.RunEnclave(ctx, enclaveOpts)
	if err != nil {
		logger.Error("error running nitro-cli run-enclave",
			zap.Error(err))
		return fmt.Errorf("failed to start enclave: %w", err)
	}

	logger.Info("started enclave",
		zap.String("name", enclaveInfo.EnclaveName),
		zap.String("enclave_id", enclaveInfo.EnclaveID),
		zap.Int("process", enclaveInfo.ProcessID))

	ticker := time.NewTicker(5 * time.Second)

	for {
		select {
		case <-ctx.Done():
			break
		case <-ticker.C:
		}

		statusInfos, err := cli.DescribeEnclaves(ctx)
		if err != nil {
			logger.Error("error running nitro-cli describe-enclaves; ignoring",
				zap.Error(err))
		}

		if len(statusInfos) == 0 {
			logger.Info("enclave appears dead, exiting")
			return fmt.Errorf("enclave exited")
		}

		enclaveFound := false

		for _, enclaveStatusInfo := range statusInfos {
			if enclaveStatusInfo.EnclaveID == enclaveInfo.EnclaveID {
				enclaveFound = true
			}
		}

		if enclaveFound {
			logger.Info("enclave heartbeat OK")
		} else {
			return fmt.Errorf("enclave exited")
		}
	}

	// Shutdown phase
	// TODO: terminate the actual enclave if needed

	// This probably won't actually be graceful because ctx is presumably already canceled,
	// otherwise we wouldn't be here.
	httpProxy.Shutdown(ctx)

	return ctx.Err()
}
