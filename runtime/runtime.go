package runtime

import (
	"context"
	"crypto/rsa"
	"crypto/x509"
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

	defaultKeyBits = 2048
)

var (
	globalRuntime       *EnclaveRuntime
	initializationError error
	initMutex           sync.Mutex
)

type EnclaveRuntime struct {
	nsm *nsm.Session
	lpk *lazyRSAKey
}

func makeRuntime() *EnclaveRuntime {
	return &EnclaveRuntime{
		lpk: makeLazyRSAKey(defaultKeyBits),
	}
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

func (runtime *EnclaveRuntime) Attest(args AttestationOptions) ([]byte, error) {
	var publicKey []byte
	var err error
	if args.PublicKey != nil && !args.NoPublicKey {
		publicKey, err = x509.MarshalPKIXPublicKey(args.PublicKey)
		if err != nil {
			return nil, err
		}
	} else if !args.NoPublicKey {
		rpk, err := runtime.GetPublicKey()
		if err != nil {
			return nil, err
		}

		publicKey, err = x509.MarshalPKIXPublicKey(rpk)
		if err != nil {
			return nil, err
		}
	}

	res, err := runtime.nsm.Send(&request.Attestation{
		Nonce:     args.Nonce,
		UserData:  args.UserData,
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

func (runtime *EnclaveRuntime) GetPublicKey() (*rsa.PublicKey, error) {
	key, err := runtime.lpk.getPrivateKey()
	if err != nil {
		return nil, err
	}

	return &key.PublicKey, nil
}

func (runtime *EnclaveRuntime) GetPrivateKey() (*rsa.PrivateKey, error) {
	key, err := runtime.lpk.getPrivateKey()
	if err != nil {
		return nil, err
	}

	return key, nil
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
