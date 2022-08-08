package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"github.com/urfave/cli/v2"
	"io"
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

	listener, err := vsock.Listen(uint32(3128))
	if err != nil {
		return err
	}

	httpProxy := proxy.MakeHTTPProxy(logger)

	go httpProxy.Serve(listener)

	// TODO: load these from the app manifest
	nitroCliArgs := []string{
		"run-enclave",
		"--cpu-count", "2",
		"--memory", "4096",
		"--eif-path", "/enclave/application.eif",
		"--enclave-cid", fmt.Sprintf("%d", cid),
	}

	if debugMode {
		nitroCliArgs = append(nitroCliArgs, "--debug-mode", "--attach-console")
	}

	cmd := exec.Command(nitroCLIExecutable, nitroCliArgs...)
	stdout := &bytes.Buffer{}
	stderr := &bytes.Buffer{}
	cmd.Stdout = stdout
	cmd.Stderr = stderr

	err = cmd.Run()
	if err != nil {
		logger.Error("error running nitro-cli run-enclave",
			zap.Error(err),
			zap.ByteString("stdout", stdout.Bytes()),
			zap.ByteString("stderr", stderr.Bytes()))

		return fmt.Errorf("failed to run enclave")
	}

	if debugMode {
		println("Debug (stdout):")
		io.Copy(os.Stdout, stderr)
		println()

		println("Debug (stderr):")
		io.Copy(os.Stdout, stdout)
		println()

		return nil
	}

	md := &EnclaveMetadata{}
	err = json.Unmarshal(stdout.Bytes(), md)

	logger.Info("enclave started",
		zap.String("name", md.EnclaveName),
		zap.String("id", md.EnclaveName),
		zap.Int("process", md.ProcessID))

	ticker := time.NewTicker(5 * time.Second)

	for {
		select {
		case <-ctx.Done():
			break
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

	// Shutdown phase
	// TODO: terminate the actual enclave if needed

	// This probably won't actually be graceful because ctx is presumably already canceled,
	// otherwise we wouldn't be here.
	httpProxy.Shutdown(ctx)

	return ctx.Err()
}

type EnclaveMetadata struct {
	EnclaveName string
	EnclaveID   string
	ProcessID   int
}
