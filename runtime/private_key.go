package runtime

import (
	"crypto/rand"
	"crypto/rsa"
	"sync"
)

type lazyRSAKey struct {
	bits       int
	privateKey *rsa.PrivateKey
	err        error
	once       sync.Once
}

func makeLazyRSAKey(bits int) *lazyRSAKey {
	return &lazyRSAKey{
		bits: bits,
	}
}

func (k *lazyRSAKey) getPrivateKey() (*rsa.PrivateKey, error) {
	k.once.Do(func() {
		key, err := rsa.GenerateKey(rand.Reader, 2048)
		if err != nil {
			k.err = err
		} else {
			k.privateKey = key
		}
	})

	return k.privateKey, k.err
}
