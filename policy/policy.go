package policy

import (
	"crypto/sha256"
	"fmt"
	"hash"
	"io/ioutil"
	"k8s.io/apimachinery/pkg/api/resource"
	"k8s.io/apimachinery/pkg/util/validation"
	"regexp"
	"sigs.k8s.io/yaml"
)

const (
	maxPort = 65535
)

var (
	minMem        = resource.MustParse("128Mi")
	appNameRegexp = regexp.MustCompile("^([A-Za-z0-9][[A-Za-z0-9_.-]*)?[A-Za-z0-9]$")
)

type ValidationError struct {
	Message string
}

func NewValidationError(msg string, a ...any) *ValidationError {
	return &ValidationError{
		Message: fmt.Sprintf(msg, a...),
	}
}

func (e *ValidationError) Error() string {
	return e.Message
}

type Policy struct {
	sourcePath string
	hash       hash.Hash
	raw        []byte
	parsed     *AppPolicy
}

func LoadPolicy(path string) (*Policy, error) {
	raw, err := ioutil.ReadFile(path)
	if err != nil {
		return nil, err
	}

	parsed := &AppPolicy{}

	err = yaml.UnmarshalStrict(raw, parsed)
	if err != nil {
		return nil, err
	}

	err = parsed.Validate()
	if err != nil {
		return nil, err
	}

	hash := sha256.New()
	_, err = hash.Write(raw)
	if err != nil {
		return nil, err
	}

	policy := &Policy{
		sourcePath: path,
		hash:       hash,
		raw:        raw,
		parsed:     parsed,
	}

	return policy, nil
}

func (policy *Policy) Raw() []byte {
	return policy.raw
}

func (policy *Policy) Size() int {
	return len(policy.raw)
}

func (policy *Policy) SHA256() []byte {
	return policy.hash.Sum(nil)
}

func (policy *Policy) Parsed() *AppPolicy {
	return policy.parsed
}

type AppPolicy struct {
	Version   string          `json:"version"`
	Name      string          `json:"name"`
	Image     string          `json:"image"`
	Resources *ResourcePolicy `json:"resources"`
	Network   *NetworkPolicy  `json:"network"`
}

func (policy *AppPolicy) Validate() error {
	if policy.Name == "" {
		return NewValidationError("name is required")
	}

	if !appNameRegexp.MatchString(policy.Name) {
		return NewValidationError("name must consist of alphanumeric characters, '-', '_' or '.' and start and end with an alphanumeric character")
	}

	if policy.Name == policy.Image {
		return NewValidationError("'name' and 'image' may not match")
	}

	if policy.Version != "v1" {
		return &ValidationError{Message: "unsupported policy version (only v1 is supported)"}
	}

	if policy.Image == "" {
		return &ValidationError{Message: "image is required"}
	}

	if policy.Resources == nil {
		return &ValidationError{Message: "resources is required"}
	}

	if err := policy.Resources.Validate(); err != nil {
		return err
	}

	return nil
}

type ResourcePolicy struct {
	CPUs int               `json:"cpus"`
	Mem  resource.Quantity `json:"memory"`
}

func (policy *ResourcePolicy) Validate() error {
	if policy.CPUs < 1 {
		return &ValidationError{Message: "cpus must be greater than 0"}
	}

	if policy.Mem.Value() < minMem.Value() {
		return &ValidationError{Message: fmt.Sprintf("memory must greater than %s", minMem.String())}
	}

	return nil
}

type NetworkPolicy struct {
	ListenPorts []int          `json:"listen_ports"`
	Egress      []EgressTarget `json:"egress"`
}

func (policy *NetworkPolicy) Validate() error {
	for _, port := range policy.ListenPorts {
		if port < 1 || port > maxPort {
			return NewValidationError("invalid port: %d", port)
		}
	}

	for _, target := range policy.Egress {
		if err := target.Validate(); err != nil {
			return err
		}
	}

	return nil
}

type EgressTarget struct {
	// Host is currently permitted to be a subdomain or IP address
	Host string `json:"host"`
	Port int    `json:"port"`
}

func (target *EgressTarget) Validate() error {
	if len(validation.IsDNS1123Subdomain(target.Host)) > 0 && len(validation.IsValidIP(target.Host)) > 0 {
		return NewValidationError("invalid host: %s", target.Host)
	}

	if target.Port < 1 || target.Port > maxPort {
		return NewValidationError("invalid port: %d", target.Port)
	}

	return nil
}
