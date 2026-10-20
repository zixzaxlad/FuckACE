import type { RestrictionSettings } from '../types/app';

interface Choices extends Partial<RestrictionSettings> {
  rememberChoices?: boolean;
  announcementsSeenKey?: string;
  autoMemoryCleanOnGameStart?: boolean;
  autoMemoryCleanThreshold?: number | null;
  affinityCores?: number[];
}

const choicesKey = 'fuckace.choices';

export const storage = {
  getChoices(): Choices {
    const saved = localStorage.getItem(choicesKey);
    return saved === null ? {} : JSON.parse(saved);
  },
  saveChoices(choices: Choices) {
    localStorage.setItem(choicesKey, JSON.stringify({ ...this.getChoices(), ...choices }));
  },
};
