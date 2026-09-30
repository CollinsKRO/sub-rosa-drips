import { getChainPassphrase, validatePassphraseMatch, getNetworkNames } from '../chain';
import { Networks } from '@stellar/stellar-sdk';

// Mock the global chain helper
global.window = {
  stellar: {
    chainPassphrase: Networks.TESTNET
  }
} as any;

describe('chain passphrase validation', () => {
  describe('getChainPassphrase', () => {
    it('returns the chain passphrase from window.stellar', () => {
      expect(getChainPassphrase()).toBe(Networks.TESTNET);
    });
  });

  describe('validatePassphraseMatch', () => {
    it('returns true when passphrases match', () => {
      expect(validatePassphraseMatch(Networks.TESTNET)).toBe(true);
    });

    it('returns false when passphrases do not match', () => {
      expect(validatePassphraseMatch(Networks.PUBLIC)).toBe(false);
    });
  });

  describe('getNetworkNames', () => {
    it('returns correct network names for known passphrases', () => {
      const result = getNetworkNames(Networks.PUBLIC);
      expect(result.chainNetwork).toBe('Testnet');
      expect(result.sdkNetwork).toBe('Public');
    });

    it('returns Unknown for unrecognized passphrases', () => {
      const customPassphrase = 'Custom Network ; September 2022';
      const result = getNetworkNames(customPassphrase);
      expect(result.chainNetwork).toBe('Testnet');
      expect(result.sdkNetwork).toBe('Unknown');
    });
  });
});