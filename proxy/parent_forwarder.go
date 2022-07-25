package proxy

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"go.uber.org/zap"
	"net"
)

type portForward struct {
	sourcePort uint32
	vsockPort  uint32
}

type ParentForwarder struct {
	logger              *zap.Logger
	listenHost          string
	vsockDestinationCID uint32
	forwards            []portForward
}

func (forwarder *ParentForwarder) StartForward(ctx context.Context) error {
	for _, pf := range forwarder.forwards {
		err := forwarder.forwardPort(ctx, pf)
		if err != nil {
			return err
		}
	}

	return nil
}

func (forwarder *ParentForwarder) forwardPort(ctx context.Context, pf portForward) error {
	listener, err := net.Listen("tcp", fmt.Sprintf("%s:%d", forwarder.listenHost, pf.sourcePort))
	if err != nil {
		return err
	}

	forwarder.logger.Info("enclave forwarder listening", zap.String("address", listener.Addr().String()))

	sfp := &StreamForwardProxy{
		logger: forwarder.logger,
		dial: func() (net.Conn, error) {
			return vsock.DialEnclave(forwarder.vsockDestinationCID, pf.vsockPort)
		},
	}

	return sfp.Serve(ctx, listener)
}
