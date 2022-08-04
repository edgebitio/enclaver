package policy

import (
	"crypto/sha256"
	"fmt"
	"hash"
	"io/ioutil"
	"k8s.io/apimachinery/pkg/api/resource"
	"sigs.k8s.io/yaml"
)

var (
	minMem = resource.MustParse("128Mi")
)

type PolicyValidationError struct {
	Message string
}

func (e *PolicyValidationError) Error() string {
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

	err = yaml.Unmarshal(raw, parsed)
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
	Version   string          `yaml:"version"`
	Image     string          `json:"image"`
	Resources *ResourcePolicy `json:"resources"`
}

func (policy *AppPolicy) Validate() error {
	if policy.Version != "v1" {
		return &PolicyValidationError{Message: "unsupported policy version (only v1 is supported)"}
	}

	if policy.Image == "" {
		return &PolicyValidationError{Message: "image is required"}
	}

	if policy.Resources == nil {
		return &PolicyValidationError{Message: "resources is required"}
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
		return &PolicyValidationError{Message: "cpus must be greater than 0"}
	}

	if policy.Mem.Value() < minMem.Value() {
		return &PolicyValidationError{Message: fmt.Sprintf("memory must greater than %s", minMem.String())}
	}

	return nil
}
