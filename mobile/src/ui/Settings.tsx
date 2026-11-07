// Grouped rows for Settings and Devices: titled sections of cards, as on desktop.

import type { ReactNode } from 'react';
import { Pressable, StyleSheet, Switch, Text, View } from 'react-native';

import { radius, text, useTheme } from '../theme';

export function Section({ title, children, footer }: { title?: string; children: ReactNode; footer?: string }) {
  const { c } = useTheme();
  return (
    <View style={styles.section}>
      {title && <Text style={[text('section'), styles.title, { color: c.ink2 }]} accessibilityRole="header">{title}</Text>}
      <View style={[styles.group, { backgroundColor: c.surface }]}>{children}</View>
      {footer && <Text style={[text('small'), styles.footer, { color: c.ink2 }]}>{footer}</Text>}
    </View>
  );
}

export function Item({
  title,
  detail,
  leading,
  trailing,
  onPress,
  destructive,
}: {
  title: string;
  detail?: string | null;
  leading?: ReactNode;
  trailing?: ReactNode;
  onPress?: () => void;
  destructive?: boolean;
}) {
  const { c } = useTheme();
  return (
    <Pressable
      onPress={onPress}
      disabled={!onPress}
      android_ripple={onPress ? { color: c.hover } : undefined}
      accessibilityRole={onPress ? 'button' : undefined}
      style={[styles.item, { borderTopColor: c.line }]}
    >
      {leading}
      <View style={styles.text}>
        <Text style={[text('setting'), { color: destructive ? c.danger : c.ink }]}>{title}</Text>
        {detail ? <Text style={[text('small'), { color: c.ink2 }]}>{detail}</Text> : null}
      </View>
      {trailing}
    </Pressable>
  );
}

export function Toggle({ value, onChange, label }: { value: boolean; onChange: (v: boolean) => void; label: string }) {
  const { c } = useTheme();
  return (
    <Switch
      value={value}
      onValueChange={onChange}
      accessibilityLabel={label}
      trackColor={{ true: c.accent, false: c.lineStrong }}
      thumbColor={c.knob}
      ios_backgroundColor={c.lineStrong}
    />
  );
}

export function Button({ title, onPress, primary }: { title: string; onPress: () => void; primary?: boolean }) {
  const { c } = useTheme();
  return (
    <Pressable
      onPress={onPress}
      accessibilityRole="button"
      style={({ pressed }) => [
        styles.button,
        { backgroundColor: primary ? c.accent : c.sunk, opacity: pressed ? 0.8 : 1 },
      ]}
    >
      <Text style={[text('bodyStrong'), { color: primary ? c.onAccent : c.ink }]}>{title}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  section: { gap: 8, marginHorizontal: 16, marginTop: 24 },
  title: { paddingHorizontal: 4 },
  group: { borderRadius: radius.card, overflow: 'hidden' },
  footer: { paddingHorizontal: 4 },
  item: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 14,
    paddingHorizontal: 18,
    paddingVertical: 14,
    minHeight: 52,
    borderTopWidth: StyleSheet.hairlineWidth,
  },
  text: { flex: 1, gap: 2 },
  button: { borderRadius: radius.control, paddingVertical: 14, paddingHorizontal: 20, alignItems: 'center' },
});
