export interface PluginVerificationResult {
  verified: boolean;
  reason?: string;
  signatureAlgorithm?: "source_binding";
  signer?: string;
  signerSource?: string;
}
