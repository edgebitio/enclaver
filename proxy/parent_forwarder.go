package proxy

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"go.uber.org/zap"
	"net"
)

type ParentForwarder struct {
	logger     *zap.Logger
	listenHost string
	dstCID     uint32
}

func MakeParentForwarder(logger *zap.Logger, listenHost string, dstCID uint32) *ParentForwarder {
	return &ParentForwarder{
		logger:     logger,
		listenHost: listenHost,
		dstCID:     dstCID,
	}
}

func (forwarder *ParentForwarder) ForwardPort(ctx context.Context, srcPort, dstPort uint32) error {
	listener, err := net.Listen("tcp", fmt.Sprintf("%s:%d", forwarder.listenHost, srcPort))
	if err != nil {
		return err
	}

	forwarder.logger.Info("enclave forwarder listening", zap.String("address", listener.Addr().String()))

	sfp := &StreamForwardProxy{
		logger: forwarder.logger,
		dial: func() (net.Conn, error) {
			return vsock.DialEnclave(forwarder.dstCID, dstPort)
		},
	}

	return sfp.Serve(ctx, listener)
}
