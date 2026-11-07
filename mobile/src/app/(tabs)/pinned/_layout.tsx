import { Stack } from 'expo-router';
import { Platform } from 'react-native';

import { text, useTheme } from '../../../theme';

export default function TabStack() {
  const { c } = useTheme();
  return (
    <Stack
      screenOptions={{
        headerLargeTitle: Platform.OS === 'ios',
        headerTransparent: Platform.OS === 'ios',
        headerShadowVisible: false,
        headerTintColor: c.accentInk,
        headerLargeTitleStyle: { fontFamily: text('pageTitle').fontFamily, color: c.ink },
        headerTitleStyle: { fontFamily: text('setting').fontFamily, color: c.ink },
        headerStyle: Platform.OS === 'android' ? { backgroundColor: c.bg } : undefined,
        contentStyle: { backgroundColor: c.bg },
      }}
    />
  );
}
