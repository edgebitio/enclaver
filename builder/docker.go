package builder

import (
	"bufio"
	"context"
	"encoding/json"
	"github.com/docker/docker/client"
	"github.com/google/go-containerregistry/pkg/name"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/tarball"
	"io"
	"io/ioutil"
)

const (
	dockerLinePrefix = "--> "
)

type statusLine struct {
	Status string `json:"status"`
}

func StreamDockerStatus(reader io.ReadCloser) {
	scanner := bufio.NewScanner(reader)
	for scanner.Scan() {
		status := statusLine{}
		line := scanner.Bytes()
		err := json.Unmarshal(scanner.Bytes(), &status)
		if err != nil {
			// Malformed JSON, just print it directly
			println(dockerLinePrefix + string(line))
		} else {
			println(dockerLinePrefix + status.Status)
		}
	}

	reader.Close()
}

func SaveImageToDocker(ctx context.Context, dockerClient *client.Client, ref name.Reference, img v1.Image) error {
	pr, pw := io.Pipe()
	go func() {
		pw.CloseWithError(tarball.Write(ref, img, pw))
	}()

	res, err := dockerClient.ImageLoad(ctx, pr, false)
	if err != nil {
		return err
	}

	defer res.Body.Close()

	// TODO: it would be nice to stream updates from this to stdout, but currently that doesn't seem to be very useful;
	// the ImageLoad call blocks for a *long* time before returning, then the stream arrives almost instantatneously.
	// It isn't clear to  me whether it is actually streaming, or possibly buffered somewhere.
	_, err = ioutil.ReadAll(res.Body)
	return err
}
