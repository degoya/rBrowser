// One set of controls for every view of a recording (RB-110-12): play, pause, seek and speed act
// on all videos at once, and `resync` pulls a video that drifted more than `tolerance` seconds
// back to the leader - the longest one - while it plays. A video shorter than the leader is left
// alone once it ended. The panel calls `resync` on the leader's `timeupdate`.

/** What the player needs of a `<video>` element. */
export interface MediaLike {
  currentTime: number;
  readonly duration: number;
  readonly paused: boolean;
  playbackRate: number;
  play(): Promise<void>;
  pause(): void;
}

const finite = (value: number): number | null => (Number.isFinite(value) && value >= 0 ? value : null);

export class SyncPlayer {
  private readonly media: MediaLike[] = [];
  private rate = 1;

  constructor(private readonly tolerance = 0.25) {}

  /** Takes a video into the set, at the set's speed. */
  attach(media: MediaLike): void {
    if (!this.media.includes(media)) {
      media.playbackRate = this.rate;
      this.media.push(media);
    }
  }

  detach(media: MediaLike): void {
    const index = this.media.indexOf(media);
    if (index >= 0) {
      this.media.splice(index, 1);
    }
  }

  /** The longest video, the first attached among equals. */
  private leader(): MediaLike | undefined {
    let lead: MediaLike | undefined;
    for (const media of this.media) {
      if (lead === undefined || (finite(media.duration) ?? -1) > (finite(lead.duration) ?? -1)) {
        lead = media;
      }
    }
    return lead;
  }

  /** The longest duration in seconds; 0 while none is known. */
  get duration(): number {
    return Math.max(0, ...this.media.map((media) => finite(media.duration) ?? 0));
  }

  /** Where the leader is. */
  get time(): number {
    return this.leader()?.currentTime ?? 0;
  }

  get playing(): boolean {
    return this.media.some((media) => !media.paused);
  }

  /** Every video is at its end. */
  private ended(): boolean {
    const known = this.media.filter((media) => finite(media.duration) !== null);
    return known.length > 0 && known.every((media) => media.currentTime >= media.duration);
  }

  /** Plays every video that has not ended - a shorter one stays at its end while the others
   * go on; from the start when all have ended. */
  async play(): Promise<void> {
    if (this.media.length === 0) {
      return;
    }
    if (this.ended()) {
      this.seek(0);
    }
    const playable = this.media.filter((media) => {
      const duration = finite(media.duration);
      return duration === null || media.currentTime < duration;
    });
    await Promise.allSettled(playable.map((media) => media.play()));
  }

  pause(): void {
    this.media.forEach((media) => media.pause());
  }

  /** Every video to `seconds`, a shorter one to its end. */
  seek(seconds: number): void {
    const target = Math.max(0, seconds);
    for (const media of this.media) {
      const duration = finite(media.duration);
      media.currentTime = duration === null ? target : Math.min(target, duration);
    }
  }

  setRate(rate: number): void {
    this.rate = rate;
    this.media.forEach((media) => {
      media.playbackRate = rate;
    });
  }

  /** Pulls the videos that drifted back to the playing leader. */
  resync(): void {
    const lead = this.leader();
    if (lead === undefined || lead.paused) {
      return;
    }
    for (const media of this.media) {
      if (media === lead) {
        continue;
      }
      const duration = finite(media.duration);
      if (duration !== null && lead.currentTime >= duration) {
        continue;
      }
      if (Math.abs(media.currentTime - lead.currentTime) > this.tolerance) {
        media.currentTime = lead.currentTime;
      }
    }
  }
}
