// "How saving works", opened from Settings › Capture.

import { Platform, ScrollView, StyleSheet, Text } from 'react-native';

import { t } from '../strings';
import { text, useTheme } from '../theme';
import { HowSaving } from '../ui/HowSaving';

export default function HowSavingScreen() {
  const { c } = useTheme();
  return (
    <ScrollView contentInsetAdjustmentBehavior="automatic" contentContainerStyle={styles.page}>
      <Text style={[text('body'), { color: c.ink2 }]}>{Platform.OS === 'ios' ? t('how.intro.ios') : t('how.intro.android')}</Text>
      <HowSaving />
      {Platform.OS === 'ios' && <Text style={[text('small'), { color: c.ink2 }]}>{t('settings.capture.ios.allow')}</Text>}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  page: { padding: 16, gap: 16, paddingBottom: 48 },
});
