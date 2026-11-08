// How clips get in and out of TailCopy on this platform (SPEC 21.4), as a short list
// of ways. Shown in onboarding and again from Settings › Capture.

import { Platform, StyleSheet, Text, View } from 'react-native';

import { t, type StringKey } from '../strings';
import { radius, text, useTheme } from '../theme';
import { Icon, type IconName } from './Icon';

const ways: Record<'ios' | 'android', [IconName, StringKey, StringKey][]> = {
  ios: [
    ['desktop', 'how.from_computer', 'how.from_computer.body'],
    ['paste', 'how.on_open', 'how.on_open.body'],
    ['arrival', 'how.share', 'how.share.ios'],
    ['settings', 'how.control', 'how.control.ios'],
  ],
  android: [
    ['desktop', 'how.from_computer', 'how.from_computer.body'],
    ['paste', 'how.on_open', 'how.on_open.body'],
    ['text', 'how.selection', 'how.selection.android'],
    ['arrival', 'how.share', 'how.share.android'],
    ['settings', 'how.tile', 'how.tile.android'],
  ],
};

export function HowSaving() {
  const { c } = useTheme();
  return (
    <View style={styles.list}>
      {ways[Platform.OS === 'ios' ? 'ios' : 'android'].map(([icon, title, body], i) => (
        <View key={title} style={[styles.way, { backgroundColor: c.surface }]} accessible>
          <View style={[styles.tile, { backgroundColor: c.accentSoft }]}>
            <Icon name={icon} size={20} color={c.accentInk} />
          </View>
          <View style={styles.words}>
            <Text style={[text('bodyStrong'), { color: c.ink }]}>
              {i + 1}. {t(title)}
            </Text>
            <Text style={[text('small'), { color: c.ink2 }]}>{t(body)}</Text>
          </View>
        </View>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  list: { gap: 10 },
  way: { flexDirection: 'row', alignItems: 'flex-start', gap: 14, padding: 14, borderRadius: radius.card },
  tile: { width: 36, height: 36, alignItems: 'center', justifyContent: 'center', borderRadius: radius.control },
  words: { flex: 1, gap: 2 },
});
