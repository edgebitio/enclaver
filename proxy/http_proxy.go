package proxy

import (
	"context"
	"fmt"
	"go.uber.org/zap"
	"io"
	"net"
	"net/http"
)

type HTTPProxy struct {
	logger *zap.Logger
	server *http.Server
}

func MakeHTTPProxy(logger *zap.Logger) *HTTPProxy {
	return &HTTPProxy{
		logger: logger,
		server: &http.Server{
			Handler: &httpProxyHandler{
				logger: logger,
			},
		},
	}
}

func (p *HTTPProxy) Serve(listener net.Listener) error {
	p.logger.Info("starting proxy", zap.String("address", listener.Addr().String()))

	return p.server.Serve(listener)
}

func (p *HTTPProxy) Shutdown(ctx context.Context) error {
	p.logger.Info("attempting graceful shutdown of proxy")

	// TODO: from the docs:
	// Shutdown does not attempt to close nor wait for hijacked
	// connections such as WebSockets. The caller of Shutdown should
	// separately notify such long-lived connections of shutdown and wait
	// for them to close, if desired. See RegisterOnShutdown for a way to
	// register shutdown notification functions.
	return p.server.Shutdown(ctx)
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
		resp, err := http.DefaultTransport.RoundTrip(req)
		if err != nil {
			p.logger.Error("error proxying request", zap.Error(err))
			http.Error(w, "Internal Error", http.StatusServiceUnavailable)
			return
		} else {
			p.logger.Info("proxying response", zap.String("status", resp.Status))
		}

		for key, values := range resp.Header {
			w.Header()[key] = values
		}

		w.WriteHeader(resp.StatusCode)

		_, err = io.Copy(w, resp.Body)
		if err != nil {
			p.logger.Error("error proxying request", zap.Error(err))
		}

		resp.Body.Close()
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

	srcConn, _, err := hijacker.Hijack()
	if err != nil {
		p.logger.Error("error hijacking connection", zap.Error(err))
		http.Error(w, "Internal Error", http.StatusInternalServerError)
		return
	}

	err = Pump(ctx, destConn, srcConn)
	if err != nil {
		p.logger.Error("error proxying connection", zap.Error(err))
	}
}
