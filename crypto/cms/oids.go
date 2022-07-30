package cms

import "encoding/asn1"

var (
	// OIDs
	OIDData                         = asn1.ObjectIdentifier{1, 2, 840, 113549, 1, 7, 1}
	OIDEnvelopedData                = asn1.ObjectIdentifier{1, 2, 840, 113549, 1, 7, 3}
	OIDEncryptionAlgorithmRSAESOAEP = asn1.ObjectIdentifier{1, 2, 840, 113549, 1, 1, 7}
	OIDEncryptionAlgorithmAES256CBC = asn1.ObjectIdentifier{2, 16, 840, 1, 101, 3, 4, 1, 42}

	// Versions
	EnvelopedDataVersion              = 2
	EnvelopedDataRecipientInfoVersion = 2
)
