package builder

import (
	"archive/tar"
	"bytes"
	"fmt"
	"github.com/go-edgebit/enclaver/policy"
	"github.com/google/go-containerregistry/pkg/name"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/daemon"
	"github.com/google/go-containerregistry/pkg/v1/mutate"
	"github.com/google/go-containerregistry/pkg/v1/tarball"
	"github.com/google/uuid"
	"io"
)

const (
	enclavePolicyFileLocation = `/etc/enclaver/policy.yaml`
)

// SourceImageToEnclaveImage takes sourceImageName, which is interpreted as a reference
// to an image in a local docker daemon, and appends a new image layer containing
// the passed Policy written out in YAML format to `/etc/enclaver/policy.yaml`.
func SourceImageToEnclaveImage(sourceImageName string, policy *policy.Policy) (any, error) {
	srcRef, err := name.ParseReference(sourceImageName)
	if err != nil {
		return nil, err
	}

	img, err := daemon.Image(srcRef)
	if err != nil {
		return nil, err
	}

	layer, err := enclaverOverlayLayer(policy)
	if err != nil {
		return nil, err
	}

	img, err = mutate.AppendLayers(img, layer)
	if err != nil {
		return nil, err
	}

	tagID, err := uuid.NewRandom()
	if err != nil {
		return nil, err
	}

	tag, err := name.NewTag(tagID.String())
	if err != nil {
		return nil, err
	}

	_, err = daemon.Write(tag, img)
	if err != nil {
		return nil, err
	}

	hash, err := img.Digest()
	if err != nil {
		return nil, err
	}

	return fmt.Sprintf("%s@%s", tag.String(), hash.String()), nil
}

// enclaverOverlayLayer generates a Layer which overlays enclaver-specific
// contents over a user-provided docker image. Today the layer contains
// only the policy YAML file, but in the future it might contain a proxy,
// process supervisor, or other common utilities.
//
// Note that if this layer gets very large this function should be refactored
// to lazily generate the layer, rather than buffering the whole thing in memory.
func enclaverOverlayLayer(policy *policy.Policy) (v1.Layer, error) {
	var tarbuf bytes.Buffer
	writer := tar.NewWriter(&tarbuf)

	err := writer.WriteHeader(&tar.Header{
		Typeflag: tar.TypeReg,
		Name:     "/etc/enclaver/policy.yaml",
		Size:     int64(policy.Size()),
		Mode:     0644,
	})
	if err != nil {
		return nil, err
	}

	_, err = writer.Write(policy.Raw())
	if err != nil {
		return nil, err
	}

	err = writer.Close()
	if err != nil {
		return nil, err
	}

	return tarball.LayerFromOpener(func() (io.ReadCloser, error) {
		// The Opener func will be called multiple times in the future, and naturally
		// each invocation must return a reader seeked to an offset of 0; so we can't
		// just pass in a reference to tarbuf, instead we must build a new Reader on each
		// invocation.
		return io.NopCloser(bytes.NewReader(tarbuf.Bytes())), nil
	})
}
