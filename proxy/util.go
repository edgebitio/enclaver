package proxy

import (
	"context"
	"errors"
	"go.uber.org/zap"
	"io"
	"net"
)

func closeWrite(conn net.Conn) {
	if tcpConn, ok := conn.(*net.TCPConn); ok {
		tcpConn.CloseWrite()
	} else {
		conn.Close()
	}
}

func closeRead(conn net.Conn) {
	if tcpConn, ok := conn.(*net.TCPConn); ok {
		tcpConn.CloseRead()
	} else {
		conn.Close()
	}
}

func Pump(ctx context.Context, a net.Conn, b net.Conn) error {
	ech := make(chan error)

	go func() {
		_, err := io.Copy(a, b)
		closeWrite(a)
		closeRead(b)
		ech <- err
	}()

	go func() {
		_, err := io.Copy(b, a)
		closeWrite(b)
		closeRead(a)
		ech <- err
	}()

	for i := 0; i < 2; i++ {
		err := <-ech
		if err != nil && !errors.Is(err, io.EOF) {
			println("pump error: ", err)
			return err
		}
	}

	return nil
}

type StreamForwardProxy struct {
	logger *zap.Logger
	dial   func() (net.Conn, error)
}

func (sfp *StreamForwardProxy) Serve(ctx context.Context, listener net.Listener) error {
	defer listener.Close()

	for {
		clientConn, err := listener.Accept()
		if err != nil {
			panic(err)
		}

		sfp.logger.Info("accepted connection", zap.String("address", clientConn.RemoteAddr().String()))

		go func() {
			serverConn, err := sfp.dial()
			if err != nil {
				panic(err)
			}

			err = Pump(ctx, clientConn, serverConn)
			if err != nil {
				sfp.logger.Warn("error pumping", zap.Error(err))
			}
		}()
	}
	return nil
}
