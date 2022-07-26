package policy

import (
	"fmt"
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

func LoadPolicy(path string) (*AppPolicy, error) {
	raw, err := ioutil.ReadFile(path)
	if err != nil {
		return nil, err
	}

	policy := &AppPolicy{}

	err = yaml.Unmarshal(raw, &policy)
	if err != nil {
		return nil, err
	}

	return policy, nil
}
