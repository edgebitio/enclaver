module github.com/go-edgebit/enclaver

go 1.18

require (
	github.com/aws/aws-sdk-go-v2 v1.16.7
	github.com/aws/aws-sdk-go-v2/config v1.15.14
	github.com/davecgh/go-spew v1.1.1
	github.com/go-edgebit/aws-sdk-go-v2/service/kms v1.18.0
	github.com/hf/nsm v0.0.0-20211106132757-1ae65a6a69ae
	github.com/mdlayher/vsock v1.1.1
	github.com/spf13/cobra v1.5.0
	github.com/vishvananda/netlink v1.1.0
	go.uber.org/zap v1.21.0
)

require (
	github.com/aws/aws-sdk-go-v2/credentials v1.12.9 // indirect
	github.com/aws/aws-sdk-go-v2/feature/ec2/imds v1.12.8 // indirect
	github.com/aws/aws-sdk-go-v2/internal/configsources v1.1.14 // indirect
	github.com/aws/aws-sdk-go-v2/internal/endpoints/v2 v2.4.8 // indirect
	github.com/aws/aws-sdk-go-v2/internal/ini v1.3.15 // indirect
	github.com/aws/aws-sdk-go-v2/service/internal/presigned-url v1.9.8 // indirect
	github.com/aws/aws-sdk-go-v2/service/kms v1.18.0 // indirect
	github.com/aws/aws-sdk-go-v2/service/sso v1.11.12 // indirect
	github.com/aws/aws-sdk-go-v2/service/sts v1.16.9 // indirect
	github.com/aws/smithy-go v1.12.0 // indirect
	github.com/fxamacker/cbor/v2 v2.2.0 // indirect
	github.com/inconshreveable/mousetrap v1.0.0 // indirect
	github.com/mdlayher/socket v0.2.0 // indirect
	github.com/spf13/pflag v1.0.5 // indirect
	github.com/vishvananda/netns v0.0.0-20191106174202-0a2b9b5464df // indirect
	github.com/x448/float16 v0.8.4 // indirect
	go.uber.org/atomic v1.7.0 // indirect
	go.uber.org/multierr v1.6.0 // indirect
	golang.org/x/net v0.0.0-20210405180319-a5a99cb37ef4 // indirect
	golang.org/x/sync v0.0.0-20210220032951-036812b2e83c // indirect
	golang.org/x/sys v0.0.0-20220722155257-8c9f86f7a55f // indirect
)

replace github.com/go-edgebit/aws-sdk-go-v2/service/kms => github.com/go-edgebit/aws-sdk-go-v2/service/kms v1.18.1-0.20220728222455-b0760c20c5fe
