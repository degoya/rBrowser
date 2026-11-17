import { describe, expect, it } from 'vitest';
import { SyncPlayer, type MediaLike } from './sync';

class FakeMedia implements MediaLike {
  currentTime = 0;
  paused = true;
  playbackRate = 1;
  played = 0;
  constructor(public duration: number) {}
  play(): Promise<void> {
    this.paused = false;
    this.played += 1;
    return Promise.resolve();
  }
  pause(): void {
    this.paused = true;
  }
}

describe('the synchronous player', () => {
  it('plays, pauses, seeks and sets the speed of every video at once', async () => {
    const player = new SyncPlayer();
    const short = new FakeMedia(10);
    const long = new FakeMedia(12);
    player.attach(short);
    player.attach(long);
    await player.play();
    expect([short.paused, long.paused]).toEqual([false, false]);
    expect(player.playing).toBe(true);
    player.pause();
    expect([short.paused, long.paused]).toEqual([true, true]);
    player.seek(11);
    expect([short.currentTime, long.currentTime]).toEqual([10, 11]);
    player.setRate(2);
    expect([short.playbackRate, long.playbackRate]).toEqual([2, 2]);
    expect(player.duration).toBe(12);
    expect(player.time).toBe(11);
  });

  it('starts again from the beginning once every video ended', async () => {
    const player = new SyncPlayer();
    const media = new FakeMedia(5);
    player.attach(media);
    media.currentTime = 5;
    await player.play();
    expect(media.currentTime).toBe(0);
  });

  it('leaves a shorter video at its end when the others play on', async () => {
    const player = new SyncPlayer();
    const short = new FakeMedia(5);
    const long = new FakeMedia(12);
    player.attach(short);
    player.attach(long);
    short.currentTime = 5;
    long.currentTime = 8;
    await player.play();
    expect(short.played).toBe(0);
    expect(short.currentTime).toBe(5);
    expect(long.played).toBe(1);
  });

  it('pulls a video that drifted back to the longest one', async () => {
    const player = new SyncPlayer(0.25);
    const lead = new FakeMedia(12);
    const late = new FakeMedia(12);
    player.attach(lead);
    player.attach(late);
    await player.play();
    lead.currentTime = 4;
    late.currentTime = 3.9;
    player.resync();
    expect(late.currentTime).toBe(3.9);
    late.currentTime = 3;
    player.resync();
    expect(late.currentTime).toBe(4);
  });

  it('leaves a video alone that ended before the others', async () => {
    const player = new SyncPlayer(0.25);
    const lead = new FakeMedia(12);
    const ended = new FakeMedia(5);
    player.attach(lead);
    player.attach(ended);
    await player.play();
    lead.currentTime = 8;
    ended.currentTime = 5;
    player.resync();
    expect(ended.currentTime).toBe(5);
  });

  it('forgets a detached video and copes with unknown durations', async () => {
    const player = new SyncPlayer();
    const media = new FakeMedia(Number.NaN);
    player.attach(media);
    expect(player.duration).toBe(0);
    player.detach(media);
    await player.play();
    expect(media.played).toBe(0);
    expect(player.time).toBe(0);
  });
});
