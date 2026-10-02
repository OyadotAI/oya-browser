/**
 * Unit tests for the Oya orb: built from placeholders, told its state, and pulsed
 * when the agent acts.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

describe('the Oya orb', () => {
  let app;
  beforeEach(() => {
    app = loadRenderer();
  });

  it('builds an orb of the asked size with a lens, a mark and a pulse, resting', () => {
    const orb = app.run("OyaOrb.create('xl')");
    assert.equal(orb.dataset.size, 'xl');
    assert.equal(orb.dataset.state, 'idle');
    assert.ok(orb.querySelector('.oya-orb-lens .oya-orb-core'));
    assert.equal(orb.querySelectorAll('.oya-orb-mark circle').length, 2);
    assert.ok(orb.querySelector('.oya-orb-pulse'));
  });

  it('takes the states it knows, and rests on any other', () => {
    const orb = app.run('OyaOrb.create()');
    app.run('OyaOrb').state(orb, 'thinking');
    assert.equal(orb.dataset.state, 'thinking');
    app.run('OyaOrb').state(orb, 'constructor');
    assert.equal(orb.dataset.state, 'idle');
  });

  it('turns a placeholder into an orb, keeping its id', () => {
    const spot = app.document.createElement('span');
    spot.dataset.orb = 'lg';
    spot.id = 'home-orb';
    app.document.body.append(spot);
    app.run('OyaOrb').mountAll();
    const orb = app.$('home-orb');
    assert.ok(orb.classList.contains('oya-orb'));
    assert.equal(orb.dataset.size, 'lg');
  });

  it('keeps a placeholder’s classes and starting state', () => {
    const spot = app.document.createElement('span');
    Object.assign(spot.dataset, { orb: 'lg', orbState: 'thinking' });
    spot.className = 'welcome-orbit';
    app.document.body.append(spot);
    app.run('OyaOrb').mountAll();
    const orb = app.document.querySelector('.welcome-orbit');
    assert.ok(orb.classList.contains('oya-orb'));
    assert.equal(orb.dataset.state, 'thinking');
  });

  it('sends a pulse that can play again', () => {
    const orb = app.run('OyaOrb.create()');
    app.run('OyaOrb').pulse(orb);
    app.run('OyaOrb').pulse(orb);
    assert.ok(orb.querySelector('.oya-orb-pulse').classList.contains('go'));
  });
});
