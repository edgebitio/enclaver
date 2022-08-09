package nitrocli

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
)

const (
	nitroCLIExecutable = "nitro-cli"
)

type NitroCLI struct{}

func (cli *NitroCLI) RunEnclave(ctx context.Context, opts RunEnclaveOptions) (*EnclaveInfo, error) {
	info := &EnclaveInfo{}
	err := cli.runAndParseJSON(ctx, opts, info)
	if err != nil {
		return nil, err
	}

	return info, nil
}

func (cli *NitroCLI) RunEnclaveDebugConsole(ctx context.Context, opts RunEnclaveOptions) (*bytes.Buffer, error) {
	cmd, err := cli.command(ctx, opts)
	stdout := &bytes.Buffer{}
	cmd.Stdout = stdout

	err = cmd.Run()
	if err != nil {
		return nil, err
	}
	out := &bytes.Buffer{}
	cmd.Stdout = out
	cmd.Stderr = out

	return out, cmd.Start()
}

func (cli *NitroCLI) DescribeEnclaves(ctx context.Context) ([]EnclaveInfo, error) {
	infos := []EnclaveInfo{}
	err := cli.runAndParseJSON(ctx, DescribeEnclavesOptions{}, infos)
	if err != nil {
		return nil, err
	}

	return infos, nil
}

func (cli *NitroCLI) command(ctx context.Context, opts argser) (*exec.Cmd, error) {
	args, err := opts.args()
	if err != nil {
		return nil, err
	}

	return exec.CommandContext(ctx, nitroCLIExecutable, args...), nil
}

func (cli *NitroCLI) runAndParseJSON(ctx context.Context, opts argser, out interface{}) error {
	cmd, err := cli.command(ctx, opts)
	if err != nil {
		return err
	}

	stdout := &bytes.Buffer{}
	cmd.Stdout = stdout

	err = cmd.Run()
	if err != nil {
		return err
	}

	err = json.Unmarshal(stdout.Bytes(), out)
	if err != nil {
		return fmt.Errorf("parsing JSON from nitro-cli: %w", err)
	}

	return nil
}

type argser interface {
	args() ([]string, error)
}
