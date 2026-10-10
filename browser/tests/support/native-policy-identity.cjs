/** Synthetic policy identities shared by unit and real-engine fixtures; no third-party brand claim. */
module.exports = function identity(platform = 'Win32') {
  const names = { Win32: 'Windows', MacIntel: 'macOS', 'Linux x86_64': 'Linux' };
  const brand = 'OyaFixture-' + platform;
  return {
    userAgent: 'OyaFixture/1.2.3.4 ' + platform,
    userAgentMetadata: {
      brands: [{ brand, version: '1' }],
      fullVersionList: [{ brand, version: '1.2.3.4' }],
      fullVersion: '1.2.3.4',
      platform: names[platform],
      platformVersion: '1.0.0',
      architecture: platform === 'MacIntel' ? 'arm' : 'x86',
      model: '',
      mobile: false,
      bitness: '64',
      wow64: false,
      formFactors: ['Desktop'],
    },
  };
};
