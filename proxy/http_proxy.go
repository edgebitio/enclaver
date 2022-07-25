package proxy

import (
	"context"
	"fmt"
	"go.uber.org/zap"
	"net"
	"net/http"
)

type HTTPProxy struct {
	logger *zap.Logger
}

func MakeHTTPProxy(logger *zap.Logger) *HTTPProxy {
	return &HTTPProxy{
		logger: logger,
	}
}

func (p *HTTPProxy) Serve(listener net.Listener) error {
	p.logger.Info("starting proxy", zap.String("address", listener.Addr().String()))

	return http.Serve(listener, &httpProxyHandler{
		logger: p.logger,
	})
}

type httpProxyHandler struct {
	logger *zap.Logger
	dialer net.Dialer
}

func (p *httpProxyHandler) ServeHTTP(w http.ResponseWriter, req *http.Request) {
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

func (p *httpProxyHandler) hijackAndProxy(ctx context.Context, w http.ResponseWriter, req *http.Request) {
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

	err = Pump(destConn, srcRW, ctx)
	if err != nil {
		p.logger.Error("error proxying connection", zap.Error(err))
	}
}
