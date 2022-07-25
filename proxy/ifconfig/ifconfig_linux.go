package ifconfig

import (
	"github.com/vishvananda/netlink"
)

func ConfigureEnclaveInterface() error {
	lo, err := netlink.LinkByName("lo")
	if err != nil {
		return err
	}

	return netlink.LinkSetUp(lo)
}
