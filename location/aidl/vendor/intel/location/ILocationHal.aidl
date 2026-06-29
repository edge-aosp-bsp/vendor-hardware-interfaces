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

package vendor.intel.location;

/**
 * Interface for reading static location from vendor HAL.
 */
@VintfStability
interface ILocationHal {
    /**
     * Get the current latitude.
     */
    double getLatitude();

    /**
     * Get the current longitude.
     */
    double getLongitude();

    /**
     * Persist a new static location. The HAL owns the vendor data store and is
     * responsible for writing the configuration, so core domains never touch
     * vendor data files directly.
     *
     * @param latitudeDegrees latitude in the range [-90.0, 90.0].
     * @param longitudeDegrees longitude in the range [-180.0, 180.0].
     */
    void setLocation(double latitudeDegrees, double longitudeDegrees);
}
