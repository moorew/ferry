// The brand icon set (design/assets/icons: 24 grid, 1.75 stroke), drawn in the
// current text colour. The only icon set the app uses (CLAUDE.md, D-031).

import Svg, { Circle, Path, Rect } from 'react-native-svg';

import { iconShapes, type IconName } from './icon-shapes';

export type { IconName };

const components = { path: Path, rect: Rect, circle: Circle } as const;

function camel(attrs: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(attrs).map(([k, v]) => [k.replace(/-(\w)/g, (_, c: string) => c.toUpperCase()), v]));
}

export function Icon({ name, size = 22, color }: { name: IconName; size?: number; color: string }) {
  return (
    <Svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color} strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round">
      {iconShapes[name].map((s, i) => {
        const Shape = components[s.tag];
        const attrs = camel(s.attrs);
        if (attrs.fill === 'currentColor') attrs.fill = color;
        return <Shape key={i} {...attrs} />;
      })}
    </Svg>
  );
}
