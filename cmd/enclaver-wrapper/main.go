package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"github.com/urfave/cli/v2"
	"math"
	"math/rand"
	"os"
	"os/exec"
	"time"

	"go.uber.org/zap"
)

const (
	nitroCLIExecutable = "nitro-cli"
)

func main() {
	app := &cli.App{
		Name:   "enclaver-wrapper",
		Usage:  "Start an enclaver application and proxy its traffic",
		Action: run,
	}

	err := app.Run(os.Args)
	if err != nil {
		fmt.Println("error: " + err.Error())
	}
}

func run(cliContext *cli.Context) error {
	ctx := context.Background()

	logger, err := zap.NewProduction()
	if err != nil {
		return err
	}

	cid := uint32(rand.Int63n(math.MaxUint32-4) + 4)

	// TODO: load all ports from the app manifest
	pf := proxy.MakeParentForwarder(logger, "localhost", cid)
	err = pf.ForwardPort(context.Background(), 8080, 8080)
	if err != nil {
		return err
	}

	listener, err := vsock.Listen(uint32(8080))
	if err != nil {
		return err
	}

	httpProxy := proxy.MakeHTTPProxy(logger)

	go httpProxy.Serve(listener)

	cmd := exec.Command(nitroCLIExecutable,
		"run-enclave",
		// TODO: load these from the app manifest
		"--cpu-count", "2",
		"--memory", "4096",
		"--eif-path", "/enclave/application.eif",
		"--enclave-cid", fmt.Sprintf("%d", cid))

	out, err := cmd.CombinedOutput()
	if err != nil {
		logger.Error("error running nitro-cli run-enclave",
			zap.Error(err),
			zap.ByteString("output", out))

		return fmt.Errorf("failed to run enclave")
	}

	println(string(out))

	ticker := time.NewTicker(5 * time.Second)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-ticker.C:
		}

		cmd := exec.Command(nitroCLIExecutable,
			"describe-enclaves")

		out, err := cmd.CombinedOutput()
		if err != nil {
			logger.Error("error running nitro-cli describe-enclaves; ignoring",
				zap.Error(err),
				zap.ByteString("output", out))
		}

		// TODO: this is an awful hack, we should parse the JSON
		if len(out) < 10 {
			logger.Info("enclave appears dead, exiting",
				zap.ByteString("output", out))
			return fmt.Errorf("enclave exited")
		}
	}

	// TODO: terminate enclave when wrapper is terminated

	return nil
}
