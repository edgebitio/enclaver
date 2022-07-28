// Package response contains commonly used constructs for NSM responses.
package response

import (
	"fmt"
	"github.com/fxamacker/cbor/v2"
)

// A Digest name.
type Digest string

// An ErrorCode string.
type ErrorCode string

// Commonly ocurring error codes.
const (
	ECSuccess          ErrorCode = "Success"
	ECInvalidArgument  ErrorCode = "InvalidArgument"
	ECInvalidResponse  ErrorCode = "InvalidResponse"
	ECReadOnlyIndex    ErrorCode = "ReadOnlyIndex"
	ECInvalidOperation ErrorCode = "InvalidOperation"
	ECBufferTooSmall   ErrorCode = "BufferTooSmall"
	ECInputTooLarge    ErrorCode = "InputTooLarge"
	ECInternalError    ErrorCode = "InternalError"
)

// Commonly ocurring digest names.
const (
	DigestSHA256 Digest = "SHA256"
	DigestSHA384 Digest = "SHA384"
	DigestSHA512 Digest = "SHA512"
)

// A DescribePCR response.
type DescribePCR struct {
	Lock bool   `cbor:"lock" json:"lock,omitempty"`
	Data []byte `cbor:"data" json:"data,omitempty"`
}

// An ExtendPCR response.
type ExtendPCR struct {
	Data []byte `cbor:"data" json:"data,omitempty"`
}

// A LockPCR response. Presence on a `Request` confirms PCR has been locked.
type LockPCR struct {
}

// A LockPCRs response. Presence on a `Request` confirms PCRs have been locked.
type LockPCRs struct {
}

// A DescribeNSM response.
type DescribeNSM struct {
	VersionMajor uint16   `cbor:"version_major" json:"version_major,omitempty"`
	VersionMinor uint16   `cbor:"version_minor" json:"version_minor,omitempty"`
	VersionPatch uint16   `cbor:"version_patch" json:"version_patch,omitempty"`
	ModuleID     string   `cbor:"module_id" json:"module_id,omitempty"`
	MaxPCRs      uint16   `cbor:"max_pcrs" json:"max_pcrs,omitempty"`
	LockedPCRs   []uint16 `cbor:"locked_pcrs" json:"digest,omitempty"`
	Digest       Digest   `cbor:"digest" json:"digest,omitempty"`
}

// An Attestation response.
type Attestation struct {
	Document []byte `cbor:"document" json:"document,omitempty"`
}

// A GetRandom response.
type GetRandom struct {
	Random []byte `cbor:"random" json:"random,omitempty"`
}

// A Response structure. One and only one field is set at any time. Always
// check the Error field first.
type Response struct {
	DescribePCR *DescribePCR `json:"DescribePCR,omitempty"`
	ExtendPCR   *ExtendPCR   `json:"ExtendPCR,omitempty"`
	LockPCR     *LockPCR     `json:"LockPCR,omitempty"`
	LockPCRs    *LockPCRs    `json:"LockPCRs,omitempty"`
	DescribeNSM *DescribeNSM `json:"DescribeNSM,omitempty"`
	Attestation *Attestation `json:"Attestation,omitempty"`
	GetRandom   *GetRandom   `json:"GetRandom,omitempty"`

	Error ErrorCode `json:"Error,omitempty"`
}

type mapResponse struct {
	DescribePCR *DescribePCR `cbor:"DescribePCR"`
	ExtendPCR   *ExtendPCR   `cbor:"ExtendPCR"`
	DescribeNSM *DescribeNSM `cbor:"DescribeNSM"`
	Attestation *Attestation `cbor:"Attestation"`
	GetRandom   *GetRandom   `cbor:"GetRandom"`

	Error ErrorCode `cbor:"Error"`
}

// UnmarshalCBOR function to correctly unmarshal the CBOR encoding according to
// Rust's cbor serde implementation.
func (res *Response) UnmarshalCBOR(data []byte) error {
	// One might try to question the sanity behind this decoding function.
	// Please enjoy this: https://github.com/pyfisch/cbor/blob/2f2d0253e2d30e5ba7812cf0b149838b0c95530d/src/ser.rs#L83-L117
	possiblyString := ""

	err := cbor.Unmarshal(data, &possiblyString)
	if nil != err {
		possiblyMap := mapResponse{}
		err := cbor.Unmarshal(data, &possiblyMap)
		if nil != err {
			return err
		}

		res.DescribePCR = possiblyMap.DescribePCR
		res.ExtendPCR = possiblyMap.ExtendPCR
		res.DescribeNSM = possiblyMap.DescribeNSM
		res.Attestation = possiblyMap.Attestation
		res.GetRandom = possiblyMap.GetRandom
		res.Error = possiblyMap.Error

		return nil
	}

	switch possiblyString {
	case "LockPCR":
		res.LockPCR = &LockPCR{}

	case "LockPCRs":
		res.LockPCRs = &LockPCRs{}

	default:
		return fmt.Errorf("Unknown NSM response with string-like value %q", possiblyString)
	}

	return nil
}
