/*
 * Copyright (C) 2026 Intel Corporation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#include <aidl/vendor/intel/location/BnLocationHal.h>

#include <mutex>
#include <string>

namespace aidl::vendor::intel::location {

class LocationHal final : public BnLocationHal {
public:
    explicit LocationHal(const std::string& configPath);

    ndk::ScopedAStatus getLatitude(double* _aidl_return) override;
    ndk::ScopedAStatus getLongitude(double* _aidl_return) override;
    ndk::ScopedAStatus setLocation(double latitudeDegrees, double longitudeDegrees);

private:
    bool readConfig();
    bool writeConfig(double latitudeDegrees, double longitudeDegrees);

    const std::string mConfigPath;
    mutable std::mutex mMutex;
    double mLatitude = 0.0;
    double mLongitude = 0.0;
};

}  // namespace aidl::vendor::intel::location
