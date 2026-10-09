/** Synthetic metadata exercises native session isolation without impersonating another browser product. */
const assert = require('node:assert/strict');
const enabled = process.env.OYA_CHECK_NATIVE_METADATA === '1';
const entropy = [
  'architecture',
  'bitness',
  'fullVersionList',
  'model',
  'platformVersion',
  'uaFullVersion',
  'wow64',
  'formFactors',
];
/** Every field differs between partitions so a host or sibling fallback is visible. */
function metadataFor(index) {
  const brand = index ? 'OyaFixtureTwo' : 'OyaFixtureOne';
  return {
    brands: [{ brand, version: index ? '2' : '1' }],
    fullVersionList: [{ brand, version: index ? '2.3.4.5' : '1.2.3.4' }],
    fullVersion: index ? '2.3.4.5' : '1.2.3.4',
    platform: index ? 'FixtureTwo' : 'FixtureOne',
    platformVersion: index ? '20.0.0' : '10.0.0',
    architecture: index ? 'x86' : 'arm',
    model: index ? 'FixtureTablet' : '',
    mobile: Boolean(index),
    bitness: index ? '32' : '64',
    wow64: Boolean(index),
    formFactors: index ? ['Tablet', 'Mobile'] : ['Desktop'],
  };
}
/** Compare actual API results including default low-entropy fields in the high-entropy response. */
function expected(index) {
  if (!enabled) return {};
  const meta = metadataFor(index);
  const low = { brands: meta.brands, mobile: meta.mobile, platform: meta.platform };
  const { fullVersion, brands: _brands, mobile: _mobile, platform: _platform, ...high } = meta;
  return { low, high: { ...low, ...high, uaFullVersion: fullVersion } };
}
/** Invalid input cannot publish partial metadata or overwrite a previously configured identity. */
function configure(jar, index) {
  if (!enabled) return;
  const value = metadataFor(index);
  const invalid = [
    null,
    1,
    [],
    {},
    { ...value, unknown: true },
    { ...value, mobile: 1 },
    { ...value, platform: '' },
    { ...value, architecture: 'x\r\ny' },
    { ...value, model: 'x'.repeat(129) },
    { ...value, brands: [] },
    { ...value, brands: [{ brand: 'x', version: 1 }] },
    { ...value, fullVersionList: [{ brand: 'Wrong', version: '1' }] },
    { ...value, formFactors: ['Invalid'] },
    { ...value, formFactors: ['Desktop', 'Desktop'] },
    { ...value, brands: Array(9).fill(value.brands[0]) },
    {
      ...value,
      brands: [value.brands[0], value.brands[0]],
      fullVersionList: [value.fullVersionList[0], value.fullVersionList[0]],
    },
  ];
  for (const input of invalid) {
    assert.throws(() => jar._setOyaUserAgentMetadata(input));
    assert.equal(jar._getOyaSessionPolicy().userAgentMetadata, undefined);
  }
  jar._setOyaUserAgentMetadata(value);
  assert.deepEqual(jar._getOyaSessionPolicy().userAgentMetadata, value);
  value.brands[0].brand = 'CallerMutation';
  value.formFactors.push('Watch');
  const readback = jar._getOyaSessionPolicy().userAgentMetadata;
  assert.deepEqual(readback, metadataFor(index));
  readback.brands[0].brand = 'SnapshotMutation';
  assert.deepEqual(jar._getOyaSessionPolicy().userAgentMetadata, metadataFor(index));
  frozen(jar, index);
}
/** Identical values remain idempotent while conflicts fail both before and after startup. */
function frozen(jar, index) {
  if (!enabled) return;
  jar._setOyaUserAgentMetadata(metadataFor(index));
  assert.throws(() => jar._setOyaUserAgentMetadata(metadataFor(index ? 0 : 1)), /cannot be changed/);
  assert.throws(() => jar._setOyaUserAgentMetadata({ ...metadataFor(index), wow64: 'false' }), /Invalid native/);
  assert.deepEqual(jar._getOyaSessionPolicy().userAgentMetadata, metadataFor(index));
}
/** A UA string is a prerequisite, and metadata cannot be added once its session is warm. */
async function checkLifecycle(session, windowFor, url) {
  if (!enabled) return;
  const bounded = session.fromPartition('metadata-bounds');
  bounded._setOyaUserAgent('OyaFixture/1 Bounds');
  const maximum = {
    ...metadataFor(0),
    model: 'x'.repeat(128),
    formFactors: ['Desktop', 'Automotive', 'Mobile', 'Tablet', 'XR', 'EInk', 'Watch'],
  };
  maximum.brands = Array.from({ length: 8 }, (_, index) => ({ brand: 'OyaFixture' + index, version: '1' }));
  maximum.fullVersionList = maximum.brands.map(({ brand }) => ({ brand, version: '1.2.3.4' }));
  bounded._setOyaUserAgentMetadata(maximum);
  assert.deepEqual(bounded._getOyaSessionPolicy().userAgentMetadata, maximum);
  const missing = session.fromPartition('metadata-no-string');
  assert.throws(() => missing._setOyaUserAgentMetadata(metadataFor(0)), /string must be installed/);
  assert.equal(missing._getOyaSessionPolicy().userAgentMetadata, undefined);
  missing._setOyaUserAgent('OyaFixture/1 Warm');
  const warm = windowFor(missing);
  await warm.loadURL(url);
  assert.throws(() => missing._setOyaUserAgentMetadata(metadataFor(0)), /before any session renderer/);
  warm.destroy();
  assert.throws(() => missing._setOyaUserAgentMetadata(metadataFor(0)), /before any session renderer/);
}
const capture = enabled
  ? `const firstLow=navigator.userAgentData.toJSON();const firstHigh=navigator.userAgentData.getHighEntropyValues(${JSON.stringify(entropy)});const withMetadata=value=>firstHigh.then(high=>({...value,low:firstLow,high}));`
  : 'const withMetadata=value=>value;';
module.exports = { enabled, configure, frozen, expected, checkLifecycle, capture };
