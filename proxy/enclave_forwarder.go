package proxy

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy/ifconfig"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"go.uber.org/zap"
	"net"
	"os"
)

const (
	enclaveForwarderListenPort = 3128
	enclaveProxyVSockPort      = 3128
)

func StartEnclaveForwarder(ctx context.Context) error {
	os.Setenv("HTTP_PROXY", fmt.Sprintf("http://localhost:%d", enclaveForwarderListenPort))
	os.Setenv("HTTPS_PROXY", fmt.Sprintf("http://localhost:%d", enclaveForwarderListenPort))

	logger, err := zap.NewProduction()
	if err != nil {
		return err
	}

	err = ifconfig.ConfigureEnclaveInterface()
	if err != nil {
		return err
	}

	listener, err := net.Listen("tcp", fmt.Sprintf(":%d", enclaveForwarderListenPort))
	if err != nil {
		return err
	}

	logger.Info("enclave forwarder listening", zap.String("address", listener.Addr().String()))

	sfp := &StreamForwardProxy{
		logger: logger,
		dial: func() (net.Conn, error) {
			return vsock.DialParent(enclaveProxyVSockPort)
		},
	}

	return sfp.Serve(ctx, listener)
}
