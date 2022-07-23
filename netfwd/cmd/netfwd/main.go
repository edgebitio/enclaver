package main

import (
	"context"
	"errors"
	"fmt"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
	"io"
	"net"
	"net/http"
)

func main() {
	rootCmd := &cobra.Command{
		Use:   "enclaver-proxy",
		Short: "Proxy traffic for an enclaver application",
		RunE: func(cmd *cobra.Command, args []string) error {
			return runProxy(3128)
		},
	}

	err := rootCmd.Execute()
	if err != nil {
		fmt.Println("error: " + err.Error())
	}
}

func runProxy(listenPort int) error {
	logger, err := zap.NewProduction()
	if err != nil {
		return err
	}

	p := &proxy{
		logger: logger,
	}

	return http.ListenAndServe(fmt.Sprintf(":%d", listenPort), p)
}

type proxy struct {
	logger *zap.Logger
	dialer net.Dialer
}

func (p *proxy) ServeHTTP(w http.ResponseWriter, req *http.Request) {
	ctx := req.Context()

	p.logger.Info("received request",
		zap.String("method", req.Method),
		zap.String("url", req.URL.String()),
		zap.String("source", req.RemoteAddr))

	switch req.Method {
	case http.MethodConnect:
		p.hijackAndProxy(ctx, w, req)
	default:
		http.Error(w, "Only CONNECT is supported by this proxy", http.StatusMethodNotAllowed)
	}
}

func (p *proxy) hijackAndProxy(ctx context.Context, w http.ResponseWriter, req *http.Request) {
	p.logger.Debug("connecting to upstream", zap.String("address", req.URL.Host))
	destConn, err := p.dialer.DialContext(ctx, "tcp", req.URL.Host)
	if err != nil {
		p.logger.Error("error connecting to upstream", zap.Error(err))
		http.Error(w, fmt.Sprintf("Error dialing: %s", req.URL.Host), http.StatusInternalServerError)
		return
	}

	defer destConn.Close()

	p.logger.Debug("connected to upstream", zap.String("address", req.URL.Host))

	w.WriteHeader(http.StatusOK)
	hijacker, ok := w.(http.Hijacker)
	if !ok {
		p.logger.Error("request is not hijackable")
		http.Error(w, "Internal Error", http.StatusInternalServerError)
		return
	}

	srcConn, srcRW, err := hijacker.Hijack()
	if err != nil {
		p.logger.Error("error hijacking connection", zap.Error(err))
		http.Error(w, "Internal Error", http.StatusInternalServerError)
		return
	}

	defer srcConn.Close()
	defer srcRW.Flush()

	errc := make(chan error)

	go func() {
		_, err := io.Copy(destConn, srcRW)
		errc <- err
	}()

	go func() {
		_, err := io.Copy(srcRW, destConn)
		errc <- err
	}()

	// TODO: cleanup error handling / logging
	for i := 0; i < 2; i++ {
		err := <-errc
		if err != nil && !errors.Is(err, io.EOF) {
			p.logger.Error("error proxying connection", zap.Error(err))
		}
	}
}
