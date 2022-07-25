package main

import (
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"github.com/spf13/cobra"
	"go.uber.org/zap"
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

	listener, err := vsock.Listen(uint32(listenPort))
	if err != nil {
		return err
	}

	httpProxy := proxy.MakeHTTPProxy(logger)

	return httpProxy.Serve(listener)
}
