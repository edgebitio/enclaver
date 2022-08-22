package main

import (
	"context"
	"fmt"
	"github.com/edgebitio/enclaver/builder"
	"github.com/edgebitio/enclaver/policy"
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
			{
				Name:  "validate-policy",
				Usage: "validate an enclaver policy file",
				Flags: []cli.Flag{
					&cli.StringFlag{
						Name:     "file",
						Aliases:  []string{"f"},
						Usage:    "Enclaver application policy is defined in `FILE`",
						Required: true,
					},
				},
				Action: ValidatePolicy,
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

	policy, err := policy.LoadPolicy(policyPath)
	if err != nil {
		return err
	}

	parsed := policy.Parsed()

	tag, err := builder.SourceImageToEnclaveImage(parsed.Image, policy)
	if err != nil {
		return err
	}

	eifInfo, err := builder.BuildEIF(ctx, tag)
	if err != nil {
		return err
	}

	imageName, err := builder.BuildEnclaveWrapperImage(ctx, eifInfo.Path, policy, cliContext.Bool("unpin-dependencies"))
	if err != nil {
		return err
	}

	fmt.Printf("successfully built image: %s\n", imageName)
	fmt.Printf("EIF Image Sha384: %s\n", eifInfo.Measurements.PCR0)

	return nil
}

func ValidatePolicy(cliContext *cli.Context) error {
	policyPath := cliContext.String("file")

	_, err := policy.LoadPolicy(policyPath)
	if err != nil {
		return err
	}

	fmt.Println("policy OK")

	return nil
}
