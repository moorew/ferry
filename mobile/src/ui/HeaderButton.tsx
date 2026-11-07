import { Pressable } from 'react-native';

import { useTheme } from '../theme';
import { Icon, type IconName } from './Icon';

export function HeaderButton({ icon, label, onPress }: { icon: IconName; label: string; onPress: () => void }) {
  const { c } = useTheme();
  return (
    <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel={label} hitSlop={10} style={{ padding: 4 }}>
      <Icon name={icon} color={c.accentInk} />
    </Pressable>
  );
}
