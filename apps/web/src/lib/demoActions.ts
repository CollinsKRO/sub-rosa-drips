import { validatePassphraseMatch, getNetworkNames } from './chain';
import { getSdkClient } from './config';

/**
 * Validates that the chain passphrase matches the SDK client network
 * @throws Error if passphrases don't match
 */
function validateNetworkPassphrase(): void {
  const sdkClient = getSdkClient();
  const sdkPassphrase = sdkClient.networkPassphrase;
  
  if (!validatePassphraseMatch(sdkPassphrase)) {
    const { chainNetwork, sdkNetwork } = getNetworkNames(sdkPassphrase);
    throw new Error(
      `Network mismatch: Chain is on ${chainNetwork} but SDK is configured for ${sdkNetwork}`
    );
  }
}

export async function commitDemoTransaction() {
  validateNetworkPassphrase();
  // Existing commit implementation
  // ...
}

export async function revealDemoTransaction() {
  validateNetworkPassphrase();
  // Existing reveal implementation
  // ...
}

export async function settleDemoTransaction() {
  validateNetworkPassphrase();
  // Existing settle implementation
  // ...
}