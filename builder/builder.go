package builder

import (
	"archive/tar"
	"bytes"
	"context"
	"fmt"
	"github.com/docker/docker/api/types"
	"github.com/docker/docker/api/types/container"
	"github.com/docker/docker/api/types/mount"
	"github.com/docker/docker/client"
	"github.com/go-edgebit/enclaver/policy"
	"github.com/google/go-containerregistry/pkg/name"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/daemon"
	"github.com/google/go-containerregistry/pkg/v1/mutate"
	"github.com/google/go-containerregistry/pkg/v1/tarball"
	"github.com/google/uuid"
	"golang.org/x/exp/slices"
	"io"
	"os"
	"path"
	"time"
)

const (
	enclavePolicyFileLocation = `/etc/enclaver/policy.yaml`
	nitroCLIContainer         = "us-docker.pkg.dev/edgebit-containers/containers/nitro-cli"
	enclaveWrapperContainer   = "us-docker.pkg.dev/edgebit-containers/containers/enclaver-wrapper-base"

	eifFilename = "application.eif"
)

var (
	allowedSourceArchitectures = []string{
		"amd64",
	}
)

// SourceImageToEnclaveImage takes sourceImageName, which is interpreted as a reference
// to an image in a local docker daemon, and appends a new image layer containing
// the passed Policy written out in YAML format to `/etc/enclaver/policy.yaml`.
func SourceImageToEnclaveImage(sourceImageName string, policy *policy.Policy) (string, error) {
	srcRef, err := name.ParseReference(sourceImageName)
	if err != nil {
		return "", err
	}

	img, err := daemon.Image(srcRef)
	if err != nil {
		return "", err
	}

	config, err := img.ConfigFile()
	if err != nil {
		return "", err
	}

	if !slices.Contains(allowedSourceArchitectures, config.Architecture) {
		return "", fmt.Errorf("unsupported source image architecture: %s", config.Architecture)
	}

	println("building overlay layer for source image")

	layer, err := enclaverOverlayLayer(policy)
	if err != nil {
		return "", err
	}

	img, err = mutate.AppendLayers(img, layer)
	if err != nil {
		return "", err
	}

	println("overlay completed, saving overlaid image")

	tagID, err := uuid.NewRandom()
	if err != nil {
		return "", err
	}

	tag, err := name.NewTag(tagID.String())
	if err != nil {
		return "", err
	}

	_, err = daemon.Write(tag, img)
	if err != nil {
		return "", err
	}

	fmt.Printf("overlaid image saved as %s\n", tagID.String())

	// Note: it would be wonderful to have a more deterministic way to refer to this image,
	// but I can't find a way to pass a digest in a format that nitro-cli or linuxkit
	// doesn't try to do a remote pull on.
	return tag.String(), nil
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
		Name:     enclavePolicyFileLocation,
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

func BuildEIF(ctx context.Context, srcImg string) (string, error) {
	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return "", err
	}

	containerRef := fmt.Sprintf("%s@%s", nitroCLIContainer, nitroCLIContainerDigest)

	println("ensuring nitro-cli container image is up to date")
	statusReader, err := dockerClient.ImagePull(ctx, containerRef, types.ImagePullOptions{})
	if err != nil {
		return "", err
	}

	StreamDockerStatus(statusReader)

	outdir, err := os.MkdirTemp("", "enclaver-build-")
	if err != nil {
		return "", err
	}

	println("building enclave EIF image")
	createResp, err := dockerClient.ContainerCreate(ctx, &container.Config{
		Image:        containerRef,
		Cmd:          []string{"build-enclave", "--docker-uri", srcImg, "--output-file", eifFilename},
		AttachStderr: true,
		AttachStdout: true,
	}, &container.HostConfig{
		Mounts: []mount.Mount{
			{
				Type:   mount.TypeBind,
				Source: "/var/run/docker.sock",
				Target: "/var/run/docker.sock",
			},
			{
				Type:   mount.TypeBind,
				Source: outdir,
				Target: "/build",
			},
		},
	}, nil, nil, "")
	if err != nil {
		return "", err
	}

	err = dockerClient.ContainerStart(ctx, createResp.ID, types.ContainerStartOptions{})
	if err != nil {
		return "", err
	}

	statusCh, errCh := dockerClient.ContainerWait(ctx, createResp.ID, container.WaitConditionNotRunning)
	select {
	case err := <-errCh:
		if err != nil {
			return "", err
		}
	case <-statusCh:
	}

	return path.Join(outdir, eifFilename), nil
}

func BuildEnclaveWrapperImage(ctx context.Context, eifPath string, policy *policy.Policy) (string, error) {
	dockerClient, err := client.NewClientWithOpts(client.FromEnv, client.WithAPIVersionNegotiation())
	if err != nil {
		return "", err
	}

	baseRef, err := name.ParseReference(fmt.Sprintf("%s@%s", enclaveWrapperContainer, enclaveWrapperContainerDigest))
	if err != nil {
		return "", err
	}

	println("ensuring wrapper base image is up to date")
	statusReader, err := dockerClient.ImagePull(ctx, baseRef.String(), types.ImagePullOptions{})
	if err != nil {
		return "", err
	}

	StreamDockerStatus(statusReader)

	img, err := daemon.Image(baseRef)
	if err != nil {
		return "", err
	}

	println("overlaying EIF and policy file onto base wrapper image")
	layer, err := wrapperOverlayLayer(eifPath, policy)
	if err != nil {
		return "", err
	}

	img, err = mutate.AppendLayers(img, layer)
	if err != nil {
		return "", err
	}

	img, err = mutate.CreatedAt(img, v1.Time{Time: time.Now()})
	if err != nil {
		return "", err
	}

	tag, err := name.NewTag(policy.Parsed().Name)
	if err != nil {
		return "", err
	}

	println("saving completed wrapper image to local docker daemon")
	err = SaveImageToDocker(ctx, dockerClient, tag, img)
	if err != nil {
		return "", err
	}

	return tag.String(), nil
}

func wrapperOverlayLayer(eifPath string, policy *policy.Policy) (v1.Layer, error) {
	var tarbuf bytes.Buffer
	writer := tar.NewWriter(&tarbuf)

	err := writer.WriteHeader(&tar.Header{
		Typeflag: tar.TypeReg,
		Name:     "/enclave/policy.yaml",
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

	eif, err := os.Open(eifPath)
	if err != nil {
		return nil, err
	}

	eifInfo, err := eif.Stat()
	if err != nil {
		return nil, err
	}

	err = writer.WriteHeader(&tar.Header{
		Typeflag: tar.TypeReg,
		Name:     "/enclave/application.eif",
		Size:     eifInfo.Size(),
		Mode:     0644,
	})
	if err != nil {
		return nil, err
	}

	_, err = io.Copy(writer, eif)
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
