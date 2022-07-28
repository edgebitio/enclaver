// Package nsm implements the Nitro Security Module interface.
package nsm

import (
	"bytes"
	"errors"
	"fmt"
	"github.com/fxamacker/cbor/v2"
	"github.com/hf/nsm/ioc"
	"github.com/hf/nsm/request"
	"github.com/hf/nsm/response"
	"os"
	"sync"
	"syscall"
	"unsafe"
)

const (
	maxRequestSize  = 0x1000
	maxResponseSize = 0x3000
	ioctlMagic      = 0x0A
)

// FileDescriptor is a generic file descriptor interface that can be closed.
// os.File conforms to this interface.
type FileDescriptor interface {
	// Provide the uintptr for the file descriptor.
	Fd() uintptr

	// Close the file descriptor.
	Close() error
}

// Options for the opening of the NSM session.
type Options struct {
	// A function that opens the NSM device file `/dev/nsm`.
	Open func() (FileDescriptor, error)

	// A function that implements the syscall.Syscall interface and is able to
	// work with the file descriptor returned from `Open` as the `a1` argument.
	Syscall func(trap, a1, a2, a3 uintptr) (r1, r2 uintptr, err syscall.Errno)
}

// DefaultOptions can be used to open the default NSM session on `/dev/nsm`.
var DefaultOptions = Options{
	Open: func() (FileDescriptor, error) {
		return os.Open("/dev/nsm")
	},
	Syscall: syscall.Syscall,
}

// ErrorIoctlFailed is an error returned when the underlying ioctl syscall has
// failed.
type ErrorIoctlFailed struct {
	// Errno is the errno returned by the syscall.
	Errno syscall.Errno
}

// Error returns the formatted string.
func (err *ErrorIoctlFailed) Error() string {
	return fmt.Sprintf("ioctl failed on device with errno %v", err.Errno)
}

// ErrorGetRandomFailed is an error returned when the GetRandom request as part
// of a `Read` has failed with an error code, is invalid or did not return any
// random bytes.
type ErrorGetRandomFailed struct {
	ErrorCode response.ErrorCode
}

// Error returns the formatted string.
func (err *ErrorGetRandomFailed) Error() string {
	if "" != err.ErrorCode {
		return fmt.Sprintf("GetRandom failed with error code %v", err.ErrorCode)
	}

	return "GetRandom response did not include random bytes"
}

var (
	// ErrSessionClosed is returned when the session is in a closed state.
	ErrSessionClosed error = errors.New("Session is closed")
)

// A Session is used to interact with the NSM.
type Session struct {
	fd      FileDescriptor
	options Options
	reqpool *sync.Pool
	respool *sync.Pool
}

type ioctlMessage struct {
	Request  syscall.Iovec
	Response syscall.Iovec
}

func send(options Options, fd uintptr, req []byte, res []byte) ([]byte, error) {
	msg := ioctlMessage{
		Request: syscall.Iovec{
			Base: &req[0],
			Len:  uint64(len(req)),
		},
		Response: syscall.Iovec{
			Base: &res[0],
			Len:  uint64(len(res)),
		},
	}

	_, _, err := options.Syscall(
		syscall.SYS_IOCTL,
		fd,
		uintptr(ioc.Command(ioc.READ|ioc.WRITE, ioctlMagic, 0, uint(unsafe.Sizeof(msg)))),
		uintptr(unsafe.Pointer(&msg)),
	)

	if 0 != err {
		return nil, &ErrorIoctlFailed{
			Errno: err,
		}
	}

	return res[:msg.Response.Len], nil
}

// OpenSession opens a new session with the provided options.
func OpenSession(opts Options) (*Session, error) {
	session := &Session{
		options: opts,
	}

	fd, err := opts.Open()
	if nil != err {
		return session, err
	}

	session.fd = fd
	session.reqpool = &sync.Pool{
		New: func() interface{} {
			return bytes.NewBuffer(make([]byte, 0, maxRequestSize))
		},
	}
	session.respool = &sync.Pool{
		New: func() interface{} {
			return make([]byte, maxResponseSize)
		},
	}

	return session, nil
}

// OpenDefaultSession opens a new session with the default options.
func OpenDefaultSession() (*Session, error) {
	return OpenSession(DefaultOptions)
}

// Close this session. It is not thread safe to Close while other threads are
// Read-ing or Send-ing.
func (sess *Session) Close() error {
	if nil == sess.fd {
		return nil
	}

	err := sess.fd.Close()
	sess.fd = nil
	sess.reqpool = nil
	sess.respool = nil

	return err
}

// Send an NSM request to the device and await its response. It safe to call
// this from multiple threads that are Read-ing or Send-ing, but not Close-ing.
// Each Send and Read call reserves at most 16KB of memory, so having multiple
// parallel sends or reads might lead to increased memory usage.
func (sess *Session) Send(req request.Request) (response.Response, error) {
	reqb := sess.reqpool.Get().(*bytes.Buffer)
	defer sess.reqpool.Put(reqb)

	reqb.Reset()
	encoder := cbor.NewEncoder(reqb)
	err := encoder.Encode(req.Encoded())
	if nil != err {
		return response.Response{}, err
	}

	resb := sess.respool.Get().([]byte)
	defer sess.respool.Put(resb)

	return sess.sendMarshaled(reqb, resb)
}

func (sess *Session) sendMarshaled(reqb *bytes.Buffer, resb []byte) (response.Response, error) {
	res := response.Response{}

	if nil == sess.fd {
		return res, errors.New("Session is closed")
	}

	resb, err := send(sess.options, sess.fd.Fd(), reqb.Bytes(), resb)
	if nil != err {
		return res, err
	}

	err = cbor.Unmarshal(resb, &res)
	if nil != err {
		return res, err
	}

	return res, nil
}

// Read entropy from the NSM device. It is safe to call this from multiple
// threads that are Read-ing or Send-ing, but not Close-ing.  This method will
// always attempt to fill the whole slice with entropy thus blocking until that
// occurs. If reading fails, it is probably an irrecoverable error.  Each Send
// and Read call reserves at most 16KB of memory, so having multiple parallel
// sends or reads might lead to increased memory usage.
func (sess *Session) Read(into []byte) (int, error) {
	reqb := sess.reqpool.Get().(*bytes.Buffer)
	defer sess.reqpool.Put(reqb)

	getRandom := request.GetRandom{}

	reqb.Reset()
	encoder := cbor.NewEncoder(reqb)
	err := encoder.Encode(getRandom.Encoded())
	if nil != err {
		return 0, err
	}

	resb := sess.respool.Get().([]byte)
	defer sess.respool.Put(resb)

	for i := 0; i < len(into); i += 0 {
		res, err := sess.sendMarshaled(reqb, resb)

		if nil != err {
			return i, err
		}

		if "" != res.Error || nil == res.GetRandom || nil == res.GetRandom.Random || 0 == len(res.GetRandom.Random) {
			return i, &ErrorGetRandomFailed{
				ErrorCode: res.Error,
			}
		}

		i += copy(into[i:], res.GetRandom.Random)
	}

	return len(into), nil
}
