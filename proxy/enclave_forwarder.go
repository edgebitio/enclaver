package proxy

import (
	"context"
	"fmt"
	"github.com/edgebitio/enclaver/proxy/ifconfig"
	"github.com/edgebitio/enclaver/proxy/vsock"
	"go.uber.org/zap"
	"net"
	"os"
)

const (
	enclaveForwarderListenPort = 3128
	enclaveProxyVSockPort      = 3128
)

func StartEnclaveForwarder(ctx context.Context, ingressPorts []int) error {
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

	err = forwardOutboundTrafficToVSock(logger)
	if err != nil {
		return err
	}

	err = forwardInboundTrafficToApp(logger, ingressPorts)
	if err != nil {
		return err
	}

	return nil
}

func forwardOutboundTrafficToVSock(logger *zap.Logger) error {
	listener, err := net.Listen("tcp", fmt.Sprintf(":%d", enclaveForwarderListenPort))
	if err != nil {
		return err
	}

	logger.Info("enclave outbound forwarder listening", zap.String("address", listener.Addr().String()))

	sfp := &StreamForwardProxy{
		logger: logger,
		dial: func() (net.Conn, error) {
			return vsock.DialParent(enclaveProxyVSockPort)
		},
	}

	go sfp.Serve(context.Background(), listener)

	return nil
}

func forwardInboundTrafficToApp(logger *zap.Logger, ingressPorts []int) error {
	for _, port := range ingressPorts {
		listener, err := vsock.Listen(uint32(port))
		if err != nil {
			return err
		}

		logger.Info("enclave inbound forwarder listening", zap.String("address", listener.Addr().String()))

		sfp := &StreamForwardProxy{
			logger: logger,
			dial: func() (net.Conn, error) {
				return net.Dial("tcp", fmt.Sprintf("localhost:%d", 8080))
			},
		}

		go sfp.Serve(context.Background(), listener)
	}

	return nil
}
