package runtime

type AttestationOptions struct {
	// Nonce is an optional cryptographic nonce which may be signed as part of the attestation
	// for use by applications in preventing replay attacks.
	Nonce []byte

	// UserData is an optional opaque blob which will be signed as part of the attestation
	// for application-defined purposes.
	UserData []byte

	// NoPublicKey will prevent the Enclaver runtime from including the runtime's default
	// public key in the attestation. By default, if no PublicKey is included in the attestation
	// options and NoPublicKey is not set to true, the runtime will pass its default public key.
	NoPublicKey bool

	// PublicKey is an optional public key which will be included in the attestation. Valid types
	// are *rsa.PublicKey, *ecdsa.PublicKey, and ed25519.PublicKey.
	PublicKey any
}
