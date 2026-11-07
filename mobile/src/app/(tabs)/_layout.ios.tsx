// iOS: the system tab bar (Liquid Glass) with History, Pinned, Devices and Search
// (SPEC 21.5). Icons come from the brand set, compiled into the asset catalog.

import { NativeTabs } from 'expo-router/native-tabs';

import { t } from '../../strings';
import { useTheme } from '../../theme';

export default function Tabs() {
  const { c } = useTheme();
  return (
    <NativeTabs tintColor={c.accent}>
      <NativeTabs.Trigger name="index">
        <NativeTabs.Trigger.Icon xcasset="tc_history" />
        <NativeTabs.Trigger.Label>{t('tab.history')}</NativeTabs.Trigger.Label>
      </NativeTabs.Trigger>
      <NativeTabs.Trigger name="pinned">
        <NativeTabs.Trigger.Icon xcasset="tc_pin" />
        <NativeTabs.Trigger.Label>{t('tab.pinned')}</NativeTabs.Trigger.Label>
      </NativeTabs.Trigger>
      <NativeTabs.Trigger name="devices">
        <NativeTabs.Trigger.Icon xcasset="tc_devices" />
        <NativeTabs.Trigger.Label>{t('tab.devices')}</NativeTabs.Trigger.Label>
      </NativeTabs.Trigger>
      <NativeTabs.Trigger name="search" role="search">
        <NativeTabs.Trigger.Label>{t('tab.search')}</NativeTabs.Trigger.Label>
      </NativeTabs.Trigger>
    </NativeTabs>
  );
}
