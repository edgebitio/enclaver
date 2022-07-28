// Package request contains constructs commonly used in the NSM request
// payload.
package request

// A Request interface.
type Request interface {
	// Returns the Go-encoded form of the request, according to Rust's cbor
	// serde.
	Encoded() interface{}
}

// A DescribePCR request.
type DescribePCR struct {
	Index uint16 `cbor:"index"`
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *DescribePCR) Encoded() interface{} {
	return map[string]*DescribePCR{
		"DescribePCR": r,
	}
}

// An ExtendPCR request.
type ExtendPCR struct {
	Index uint16 `cbor:"index"`
	Data  []byte `cbor:"data"`
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *ExtendPCR) Encoded() interface{} {
	return map[string]*ExtendPCR{
		"ExtendPCR": r,
	}
}

// A LockPCR request.
type LockPCR struct {
	Index uint16 `cbor:"index"`
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *LockPCR) Encoded() interface{} {
	return map[string]*LockPCR{
		"LockPCR": r,
	}
}

// A LockPCRs request.
type LockPCRs struct {
	Range uint16 `cbor:"range"`
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *LockPCRs) Encoded() interface{} {
	return map[string]*LockPCRs{
		"LockPCRs": r,
	}
}

// A DescribeNSM request.
type DescribeNSM struct {
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *DescribeNSM) Encoded() interface{} {
	return "DescribeNSM"
}

// An Attestation request.
type Attestation struct {
	UserData  []byte `cbor:"user_data"`
	Nonce     []byte `cbor:"nonce"`
	PublicKey []byte `cbor:"public_key"`
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *Attestation) Encoded() interface{} {
	return map[string]*Attestation{
		"Attestation": r,
	}
}

// A GetRandom request.
type GetRandom struct {
}

// Encoded returns the Go-encoded form of the request, according to Rust's cbor
// serde.
func (r *GetRandom) Encoded() interface{} {
	return "GetRandom"
}
