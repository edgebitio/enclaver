package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/builder"
	policy2 "github.com/go-edgebit/enclaver/policy"
	"github.com/urfave/cli/v2"
	"os"
)

func main() {
	app := &cli.App{
		Name: "enclaver",
		Commands: []*cli.Command{
			{
				Name:  "build",
				Usage: "build an enclaver image",
				Flags: []cli.Flag{
					&cli.StringFlag{
						Name:     "file",
						Aliases:  []string{"f"},
						Usage:    "Enclaver application policy is defined in `FILE`",
						Required: true,
					},
					&cli.BoolFlag{
						Name:  "unpin-dependencies",
						Usage: "use the latest available version of dependencies, instead of statically pinned version (not recommended)",
					},
				},
				Action: ExecuteBuild,
			},
		},
	}

	err := app.Run(os.Args)
	if err != nil {
		fmt.Println("error: " + err.Error())
	}
}

func ExecuteBuild(cliContext *cli.Context) error {
	ctx := context.Background()

	policyPath := cliContext.String("file")

	policy, err := policy2.LoadPolicy(policyPath)
	if err != nil {
		return err
	}

	parsed := policy.Parsed()

	tag, err := builder.SourceImageToEnclaveImage(parsed.Image, policy)
	if err != nil {
		return err
	}

	eifPath, err := builder.BuildEIF(ctx, tag)
	if err != nil {
		return err
	}

	imageName, err := builder.BuildEnclaveWrapperImage(ctx, eifPath, policy, cliContext.Bool("unpin-dependencies"))
	if err != nil {
		return err
	}

	fmt.Printf("successfully built image: %s\n", imageName)

	return nil
}
