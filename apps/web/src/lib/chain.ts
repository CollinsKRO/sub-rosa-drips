import { Networks } from '@stellar/stellar-sdk';

/**
 * Gets the current chain passphrase from the web chain helper
 */
export function getChainPassphrase(): string {
  // Implementation depends on existing chain helper
  // This is a placeholder for the actual implementation
  return window.stellar?.chainPassphrase || Networks.TESTNET;
}

/**
 * Compares the chain passphrase with the SDK client network passphrase
 * @param sdkPassphrase The passphrase from the SDK client network
 * @returns true if passphrases match, false otherwise
 */
export function validatePassphraseMatch(sdkPassphrase: string): boolean {
  const chainPassphrase = getChainPassphrase();
  return chainPassphrase === sdkPassphrase;
}

/**
 * Gets both network names for error display
 * @param sdkPassphrase The passphrase from the SDK client network
 * @returns Object containing both network names
 */
export function getNetworkNames(sdkPassphrase: string): { chainNetwork: string; sdkNetwork: string } {
  const chainPassphrase = getChainPassphrase();
  
  function getNetworkName(passphrase: string): string {
    if (passphrase === Networks.PUBLIC) return 'Public';
    if (passphrase === Networks.TESTNET) return 'Testnet';
    if (passphrase === Networks.FUTURENET) return 'Futurenet';
    if (passphrase === Networks.STANDALONE) return 'Standalone';
    return 'Unknown';
  }
  
  return {
    chainNetwork: getNetworkName(chainPassphrase),
    sdkNetwork: getNetworkName(sdkPassphrase)
  };
}