package proxy

import (
	"context"
	"errors"
	"golang.org/x/sync/errgroup"
	"io"
)

func Pump(a io.ReadWriter, b io.ReadWriter, ctx context.Context) error {
	eg, _ := errgroup.WithContext(ctx)

	eg.Go(func() error {
		_, err := io.Copy(a, b)
		return err
	})

	eg.Go(func() error {
		_, err := io.Copy(b, a)
		return err
	})

	err := eg.Wait()
	if errors.Is(err, io.EOF) {
		return nil
	} else {
		return err
	}
}
