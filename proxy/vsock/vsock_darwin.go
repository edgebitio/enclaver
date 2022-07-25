package vsock

import (
	"fmt"
	"net"
)

// To improve development velocity, we abstract away vsocks behind this simple interface,
// and provide a Darwin implementation that just uses TCP over localhost, and adds a
// random-ish constant value to the requested ports to reduce the likelihood of collisions.

const (
	tcpVSockSimulationPortOffset = 3573
)

func DialParent(port uint32) (net.Conn, error) {
	return net.Dial("tcp", fmt.Sprintf("localhost:%d", port+tcpVSockSimulationPortOffset))
}

func Listen(port uint32) (net.Listener, error) {
	return net.Listen("tcp", fmt.Sprintf("localhost:%d", port+tcpVSockSimulationPortOffset))
}
