// Shown when Tailscale isn't connected: the phone can't reach anything (SPEC 21.3.1).

import { StyleSheet, Text, View } from 'react-native';

import { Core } from '../../modules/tailcopy-core';
import { useSyncStatus } from '../core/hooks';
import { t } from '../strings';
import { radius, text, useTheme } from '../theme';
import { Icon } from './Icon';
import { Button } from './Settings';

export function TailnetBanner() {
  const { c } = useTheme();
  const status = useSyncStatus();
  if (status.tailnet) return null;
  return (
    <View style={[styles.banner, { backgroundColor: c.sunk }]} accessibilityRole="alert">
      <View style={styles.head}>
        <Icon name="offline" color={c.ink2} />
        <Text style={[text('setting'), { color: c.ink }]}>{t('tailscale.off.title')}</Text>
      </View>
      <Text style={[text('small'), { color: c.ink2 }]}>{t('tailscale.off.body')}</Text>
      <View style={styles.buttons}>
        <Button primary title={t('tailscale.open')} onPress={() => Core.openTailscale()} />
        <Button title={t('tailscale.retry')} onPress={() => Core.retry()} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  banner: { margin: 16, padding: 16, gap: 10, borderRadius: radius.card },
  head: { flexDirection: 'row', alignItems: 'center', gap: 10 },
  buttons: { flexDirection: 'row', gap: 10 },
});
