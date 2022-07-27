package runtime

import (
	"context"
	"errors"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/hf/nsm"
	"github.com/hf/nsm/request"
	"os"
	"sync"
)

const (
	initializationEntropy = 1024
	randomDevice          = "/dev/random"
)

var (
	globalRuntime       *EnclaveRuntime
	initializationError error
	initMutex           sync.Mutex
)

type EnclaveRuntime struct {
	nsm *nsm.Session
}

func (runtime *EnclaveRuntime) initialize() error {
	err := proxy.StartEnclaveForwarder(context.Background())
	if err != nil {
		return err
	}

	runtime.nsm, err = nsm.OpenDefaultSession()
	if err != nil {
		return err
	}

	err = runtime.initializeEntropy()
	if err != nil {
		return err
	}

	return nil
}

// https://github.com/aws/aws-nitro-enclaves-sdk-c/blob/46a564270e5713559116833fe1303eab4c4bad0d/source/nitro_enclaves.c#L61
func (runtime *EnclaveRuntime) initializeEntropy() error {
	devRand, err := os.OpenFile(randomDevice, os.O_WRONLY, 0)
	if err != nil {
		return err
	}

	buf := make([]byte, initializationEntropy)
	_, err = runtime.nsm.Read(buf)
	if err != nil {
		return err
	}

	_, err = devRand.Write(buf)
	if err != nil {
		return err
	}

	return nil
}

func (runtime *EnclaveRuntime) Attest(nonce, userData, publicKey []byte) ([]byte, error) {
	res, err := runtime.nsm.Send(&request.Attestation{
		Nonce:     nonce,
		UserData:  userData,
		PublicKey: publicKey,
	})
	if err != nil {
		return nil, err
	}

	if res.Error != "" {
		return nil, errors.New(string(res.Error))
	}

	if res.Attestation == nil || res.Attestation.Document == nil {
		return nil, errors.New("attestation response missing attestation document")
	}

	return res.Attestation.Document, nil
}

func GetOrInitialize() (*EnclaveRuntime, error) {
	// This could be optimized with an RWMutex, at the expense of readability. Callers
	// are expected to cache the result of this operation. Given the overall performance
	// characteristics of this operation it doesn't seem worth it right now.
	initMutex.Lock()
	defer initMutex.Unlock()

	if globalRuntime == nil && initializationError == nil {
		runtime := &EnclaveRuntime{}
		if err := runtime.initialize(); err != nil {
			initializationError = err
		} else {
			globalRuntime = runtime
		}
	}

	return globalRuntime, initializationError
}
