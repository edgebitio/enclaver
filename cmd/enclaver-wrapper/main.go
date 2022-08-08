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
		println("non-zero exit code from nitro-cli:")
		os.Stdout.Write(out)
		println()
		return fmt.Errorf("failed to run enclave")
	}

	println(string(out))

	// TODO: terminate enclave when wrapper is terminated
	// TODO: terminate wrapper when enclave is terminated

	return nil
}
