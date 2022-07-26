package main

import (
	"fmt"
	"github.com/davecgh/go-spew/spew"
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
	policyPath := cliContext.String("file")

	policy, err := policy2.LoadPolicy(policyPath)
	if err != nil {
		return err
	}

	err = policy.Validate()
	if err != nil {
		return err
	}

	spew.Dump(policy)
	return nil
}
