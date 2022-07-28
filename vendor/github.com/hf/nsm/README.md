# Nitro Security Module Interface for Go

[![Go Report Card][go-reportcard-badge]][go-reportcard] [![Go Reference][pkg.go.dev-badge]][pkg.go.dev]

This is an implementation of the [AWS Nitro Security Module][nsm] interface for
Go. Nitro Enclaves only support Linux OSs on X86 or X64, so this package is not
intended to be used on other OSs or architectures.

## Usage

You can import the package like so:

```go
import (
    "github.com/hf/nsm"
)
```

The NSM interface implements a request/response communication model. Consult
the `request` and `response` subpackages for all possible requests and
responses.

You open a communication channel with the NSM device by opening a new NSM
`Session` like so:

```go
sess, err := nsm.OpenDefaultSession()
```

Use the `Send` function on the `Session` to send a new `Request` and receive
its `Response`. The response struct always has one and only one field set with
a non-zero value. Regardless, always check the `Error` field for a possible
error message returned from the NSM driver.

### Performance

You can open as many sessions as the OS will allow. You can send as many
requests on any session, from as many goroutines as resources allow. Sending
to, reading from and closing a session at the same time is not thread safe;
sending requests, and reading entropy at the same time is thread safe.

Each `Send` and `Receive` reserve 16KB of memory per call. This is the way the
NSM IOCTL interface is designed, so memory exhaustion may occur if you send a
request or read entropy at once from many threads. Memory allocations are
amortized across multiple invocations, so GC pressure should not be a
significant concern.

Since the underlying transport is an IOCTL, each call performs a syscall with a
blocking context switch on that thread. The NSM driver also context-switches on
the Nitro hypervisor, so each request is quite expensive. Use them sparingly.
For example, ask for an attestation only a couple of times within the
implementation of some protocol; use the random entropy to seed a [NIST
SP800-90A DRBG][nist-sp800-90a].

### Reading Entropy

Nitro Enclaves don't get access to `/dev/random` or `/dev/urandom`, but you can
use the NSM to generate cryptographically secure pseudo-random numbers
(entropy). A `Session` is also an `io.Reader` that asks the NSM for random
bytes.

Here's an example how you can use the NSM for entropy:

```go
import (
    "crypto/rand"
    "math/big"
    "github.com/hf/nsm"
)

func generateBigPrime() (*big.Int, error) {
    sess, err := nsm.OpenDefaultSession()
    defer sess.Close()

    if nil != err {
        return nil, err
    }

    return rand.Prime(sess, 2048)
}
```

### Obtaining an Attestation Document

Here's an example of how you can get an [attestation 
document][aws-nitro-attestation]:

```go
import (
    "errors"
    "github.com/hf/nsm"
    "github.com/hf/nsm/request"
)

func attest(nonce, userData, publicKey []byte) ([]byte, error) {
    sess, err := nsm.OpenDefaultSession()
    defer sess.Close()

    if nil != err {
        return nil, err
    }

    res, err := sess.Send(&request.Attestation{
        Nonce: nonce,
        UserData: userData,
        PublicKey: publicKey,
    })
    if nil != err {
        return nil, err
    }

    if "" != res.Error {
        return nil, errors.New(string(res.Error))
    }

    if nil == res.Attestation || nil == res.Attestation.Document {
        return nil, errors.New("NSM device did not return an attestation")
    }

    return res.Attestation.Document, nil
}
```

There's a full example in `example/attestation`.

## Reference Implementation

This implementation is based on the [Nitro Enclaves SDK][nitro-enclaves-sdk]
from AWS, which is written in Rust. This implementation is a pure Go
implementation of the same interface; thus you can use it to prepare
reproducible builds without relying on `cgo`.

## License

Copyright &copy; 2021 Stojan Dimitrovski. Licensed under the MIT License. See
`LICENSE` for more information.

[go-reportcard-badge]: https://goreportcard.com/badge/github.com/hf/nsm
[go-reportcard]: https://goreportcard.com/report/github.com/hf/nsm
[pkg.go.dev-badge]: https://pkg.go.dev/badge/github.com/hf/nsm.svg
[pkg.go.dev]: https://pkg.go.dev/github.com/hf/nsm

[nsm]: https://github.com/aws/aws-nitro-enclaves-nsm-api
[aws-nitro-attestation]: https://docs.aws.amazon.com/enclaves/latest/user/set-up-attestation.html
[nitro-enclaves-sdk]: https://github.com/aws/aws-nitro-enclaves-nsm-api
[nist-sp800-90a]: https://csrc.nist.gov/publications/detail/sp/800-90a/rev-1/final
