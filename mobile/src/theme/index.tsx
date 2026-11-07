// The Lilac theme for React Native (SPEC 21.5): generated tokens, light and dark by
// the system setting. Lilac only; no wallpaper colours on Android (D-031).

import { createContext, useContext, type ReactNode } from 'react';
import { Platform, useColorScheme, type TextStyle } from 'react-native';

import { dark, deviceColours, light, type Palette, type } from './tokens';

export { motion, qr, radius } from './tokens';

export interface Theme {
  dark: boolean;
  c: Palette;
}

const ThemeContext = createContext<Theme>({ dark: false, c: light });

export function ThemeProvider({ children }: { children: ReactNode }) {
  const scheme = useColorScheme();
  const value = scheme === 'dark' ? { dark: true, c: dark } : { dark: false, c: light };
  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): Theme {
  return useContext(ThemeContext);
}

/** A device's colour. Always drawn next to the device's name. */
export function deviceColour(index: number): string {
  return deviceColours[index % deviceColours.length];
}

type Family = 'display' | 'ui' | 'mono';

// iOS finds a font by family and weight; Android by the file name of each cut.
const androidCuts: Record<Family, Record<number, string>> = {
  display: { 800: 'BricolageGrotesque-ExtraBold' },
  ui: { 400: 'Figtree-Regular', 500: 'Figtree-Medium', 600: 'Figtree-SemiBold', 700: 'Figtree-Bold' },
  mono: { 400: 'JetBrainsMono-Regular' },
};
const iosFamilies: Record<Family, string> = {
  display: 'Bricolage Grotesque',
  ui: 'Figtree',
  mono: 'JetBrains Mono',
};

/** A type style from design/tokens as a React Native text style. */
export function text(style: keyof typeof type, weight?: number): TextStyle {
  const s = type[style] as { family: Family; size: number; weight: number; lineHeight?: number; tracking?: number; uppercase?: boolean };
  const w = weight ?? s.weight;
  return {
    fontSize: s.size,
    lineHeight: s.lineHeight,
    letterSpacing: s.tracking,
    textTransform: s.uppercase ? 'uppercase' : undefined,
    ...(Platform.OS === 'android'
      ? { fontFamily: androidCuts[s.family][w] ?? androidCuts[s.family][400] ?? androidCuts[s.family][800] }
      : { fontFamily: iosFamilies[s.family], fontWeight: String(w) as TextStyle['fontWeight'] }),
  };
}
