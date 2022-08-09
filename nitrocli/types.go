package nitrocli

import "fmt"

type RunEnclaveOptions struct {
	CPUCount int
	Memory   int
	EIFPath  string
	CID      uint32
}

func (opts RunEnclaveOptions) args() ([]string, error) {
	nitroCliArgs := []string{"run-enclave"}

	if opts.CPUCount < 1 {
		return nil, fmt.Errorf("at least 1 CPU is required, got: %d", opts.CPUCount)
	} else {
		nitroCliArgs = append(nitroCliArgs, "--cpu-count", fmt.Sprintf("%d", opts.CPUCount))
	}

	if opts.Memory < 64 {
		return nil, fmt.Errorf("at least 64MiB of memory are required, got: %d", opts.Memory)
	} else {
		nitroCliArgs = append(nitroCliArgs, "--memory", fmt.Sprintf("%d", opts.Memory))
	}

	if opts.EIFPath == "" {
		return nil, fmt.Errorf("missing EIF path")
	} else {
		nitroCliArgs = append(nitroCliArgs, "--eif-path", opts.EIFPath)
	}

	if opts.CID != 0 {
		nitroCliArgs = append(nitroCliArgs, "--enclave-cid", fmt.Sprintf("%d", opts.CID))
	}

	return nitroCliArgs, nil
}

type DescribeEnclavesOptions struct{}

func (opts DescribeEnclavesOptions) args() ([]string, error) {
	return []string{"describe-enclaves"}, nil
}

type EnclaveInfo struct {
	EnclaveName string
	EnclaveID   string
	ProcessID   int
}

type EIFInfo struct {
	Measurements Measurements
}

type Measurements struct {
	PCR0 string
	PCR1 string
	PCR2 string
}
