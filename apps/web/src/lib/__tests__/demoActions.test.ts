import {
  commitDemoTransaction,
  revealDemoTransaction,
  settleDemoTransaction
} from '../demoActions';
import { Networks } from '@stellar/stellar-sdk';

// Mock the chain helper
global.window = {
  stellar: {
    chainPassphrase: Networks.TESTNET
  }
} as any;

// Mock the SDK client
jest.mock('../config', () => ({
  getSdkClient: () => ({
    networkPassphrase: Networks.TESTNET
  })
}));

describe('demo actions with passphrase validation', () => {
  describe('when passphrases match', () => {
    it('allows commitDemoTransaction', async () => {
      await expect(commitDemoTransaction()).resolves.not.toThrow();
    });

    it('allows revealDemoTransaction', async () => {
      await expect(revealDemoTransaction()).resolves.not.toThrow();
    });

    it('allows settleDemoTransaction', async () => {
      await expect(settleDemoTransaction()).resolves.not.toThrow();
    });
  });

  describe('when passphrases do not match', () => {
    beforeEach(() => {
      // Override the SDK client mock to return different passphrase
      jest.resetModules();
      jest.doMock('../config', () => ({
        getSdkClient: () => ({
          networkPassphrase: Networks.PUBLIC
        })
      }));
    });

    it('blocks commitDemoTransaction with network names in error', async () => {
      await expect(commitDemoTransaction()).rejects.toThrow(
        'Network mismatch: Chain is on Testnet but SDK is configured for Public'
      );
    });

    it('blocks revealDemoTransaction with network names in error', async () => {
      await expect(revealDemoTransaction()).rejects.toThrow(
        'Network mismatch: Chain is on Testnet but SDK is configured for Public'
      );
    });

    it('blocks settleDemoTransaction with network names in error', async () => {
      await expect(settleDemoTransaction()).rejects.toThrow(
        'Network mismatch: Chain is on Testnet but SDK is configured for Public'
      );
    });

    it('does not include secrets or XDR in error message', async () => {
      try {
        await commitDemoTransaction();
      } catch (error) {
        const errorMessage = (error as Error).message;
        expect(errorMessage).not.toMatch(/secret/i);
        expect(errorMessage).not.toMatch(/xdr/i);
        expect(errorMessage).not.toContain('S');
        expect(errorMessage).not.toContain('AAAA');
      }
    });
  });
});